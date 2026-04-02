use async_openai::types::chat::ChatCompletionResponseMessage;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn new_system(content: String) -> Self {
        Self {
            role: Role::System,
            content,
        }
    }

    pub fn new_user(content: String) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
       write!(f, "{}: {}", self.role, self.content) 
    }
}

impl From<ChatCompletionResponseMessage> for Message {
    fn from(value: ChatCompletionResponseMessage) -> Self {
        let role = Role::from(value.role);
        let content = value.content.unwrap_or_default();
        Self { role, content }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    System,
    Assistant,
    Tool,
}

impl From<async_openai::types::chat::Role> for Role {
    fn from(value: async_openai::types::chat::Role) -> Self {
        match value {
            async_openai::types::chat::Role::User => Self::User,
            async_openai::types::chat::Role::System => Self::System,
            async_openai::types::chat::Role::Assistant => Self::Assistant,
            async_openai::types::chat::Role::Tool => Self::Tool,
            async_openai::types::chat::Role::Function => unimplemented!(),
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let role = match self {
           Role::System => "System",
           Role::User => "User",
           Role::Assistant => "Assistant",
           Role::Tool => "Tool",
        };
        write!(f, "{role}")
    }
} 
