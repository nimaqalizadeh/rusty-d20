use async_openai::types::chat::{ChatCompletionMessageToolCalls, ChatCompletionResponseMessage};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn new_system(content: String) -> Self {
        Self {
            role: Role::System,
            content,
            tool_calls: None,
        }
    }

    pub fn new_user(content: String) -> Self {
        Self {
            role: Role::User,
            content,
            tool_calls: None,
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
        let tool_calls = value.tool_calls.map(|response_tool_calls| {
            response_tool_calls
                .into_iter()
                .map(|response_tool_call| ToolCall::from(response_tool_call))
                .collect()
        });
        Self {
            role,
            content,
            tool_calls,
        }
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

impl From<ChatCompletionMessageToolCalls> for ToolCall {
    fn from(value: ChatCompletionMessageToolCalls) -> Self {
        match value {
            ChatCompletionMessageToolCalls::Function(chat_completion_message_tool_call) => {
                let id = chat_completion_message_tool_call.id;
                let tool_call_type = "function".to_owned();
                todo!()
            }
            ChatCompletionMessageToolCalls::Custom(_) => todo!(),
        }
    }
}

impl From<async_openai::types::assistants::FunctionCall> for ToolCallFunction {
    fn from(value: async_openai::types::assistants::FunctionCall) -> Self {
        let name = value.name;
        let arguments = value.arguments;
        Self { name, arguments }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(rename = "type")]
    pub tool_call_type: String,
    pub id: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}
