use serde::{Deserialize, Serialize};

use crate::{message::Message, tools};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Value>,
}

impl Context {
    pub fn new(model: String) -> Self {
        let system = Message::new_system(
            "You are a friendly AI assistant who uses tools to solve problems".to_owned(),
        );
        let messages = vec![system];
        let available_tools = vec![tools::random_number::create_tool()];

        Self {
            model,
            messages,
            tools: available_tools,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message)
    }
}
