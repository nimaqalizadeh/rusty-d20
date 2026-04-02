use serde::{Deserialize, Serialize};

use crate::message::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub model: String,
    pub messages: Vec<Message>
}

impl Context {
    pub fn new(model: String) -> Self {
        let system = Message::new_system(
            "You are a friendly AI assistant who uses tools to solve problems".to_owned());
        let messages = vec![system]; 
        Self { model, messages}
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message)
    }
}
