mod context;
mod message;

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
        context.add_message({Message::new_user(user_prompt)
});
        let client_response = sent_to_ai(&context,&client).await?;
    }
}

pub fn get_user_prompt() -> Result<String> {
    let mut prompt = String::new();

    println!("> ");
    stdout().flush()?;
    stdin().read_line(&mut prompt)?;

    Ok(prompt)
}

pub async fn sent_to_ai(context: &AIContext, 
                        client: &Client<OpenAIConfig>) -> Result<Message> {
    let response: CreateChatCompletionResponse = client
                        .chat()
                        .create_byot(context)
                        .await
                        .context("Sending message to AI")?;
    
    let choice = response.choices[0].clone();

    let message = Message::from(choice.message);
    Ok(message)

}
