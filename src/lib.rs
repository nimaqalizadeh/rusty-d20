mod context;
mod message;
mod tools;

use crate::{context::Context as AIContext, message::Message};
use async_openai::{Client, config::OpenAIConfig, types::chat::CreateChatCompletionResponse};
use eyre::{Context, Result};
use std::io::{Write, stdin, stdout};

pub async fn run(model: String, api_key: String, api_base: String) -> Result<()> {
    let mut context = AIContext::new(model);

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(api_base);

    let client = Client::with_config(config);

    loop {
        let user_prompt = get_user_prompt()?;
        let user_message = Message::new_user(user_prompt);
        println!("{user_message}");

        context.add_message(user_message);

        let ai_message = send_to_ai(&context, &client).await?;

        if !ai_message.content.is_empty() {
            println!("{ai_message}");
        }

        context.add_message(ai_message.clone());

        while context.messages.last().is_some_and(|message| {
            message
                   .tool_calls
                   .clone()
                   .is_some_and(|tool_calls| !tool_calls.is_empty())
        }) {

        if let Some(tool_calls) = context.messages.last().cloned().unwrap().tool_calls {
            for tool_call in tool_calls {
                let name = tool_call.function.name.as_str();

                let id = tool_call.id;
                let result = match name {
                    tools::random_number::NAME => {
                        tools::random_number::run(tool_call.function.arguments, id)
                    }
                    _ => Message::new_tool(format!("Error, the tool {name} doesn't exist"), id),
                };
                context.add_message(result);
            }
            let ai_tool_response = send_to_ai(&context, &client).await?;
            println!("{ai_tool_response}");
            context.add_message(ai_tool_response);
            }
        }
    }
}

pub fn get_user_prompt() -> Result<String> {
    let mut prompt = String::new();

    print!("> ");
    stdout().flush()?;
    stdin().read_line(&mut prompt)?;

    Ok(prompt.trim().to_owned())
}

pub async fn send_to_ai(context: &AIContext, client: &Client<OpenAIConfig>) -> Result<Message> {
    let response: CreateChatCompletionResponse = client
        .chat()
        .create_byot(context)
        .await
        .context("Sending message to AI")?;

    let choice = response.choices[0].clone();

    let message = Message::from(choice.message);
    Ok(message)
}
