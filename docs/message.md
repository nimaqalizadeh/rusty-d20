# message.rs

`message.rs` defines the types that represent every message in a conversation: user prompts, system instructions, assistant replies, and tool results. It also bridges between the app's own domain types and the `async_openai` crate's API types, so the rest of the codebase never touches library internals directly.

---

## The API Contract

rusty-d20 talks to an OpenAI-compatible chat completions endpoint (in this case, Ollama at `localhost:11434/v1`). Every type in `message.rs` exists to either **serialize into** the request JSON or **deserialize from** the response JSON. Here are both sides of a real exchange:

### Request

```json
{
    "model": "qwen3:8b",
    "messages": [
        { "role": "system", "content": "You are a friendly AI assistant who uses tools to solve problems" },
        { "role": "user", "content": "roll 1d6" }
    ],
    "tools": [
        {
            "type": "function",
            "function": {
                "name": "random_numbers",
                "description": "Generate a random integer between 'min' and 'max'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "min": {
                            "type": "number",
                            "description": "The lowest possible number that can be generated. For example 0."
                        },
                        "max": {
                            "type": "number",
                            "description": "The largest possible number that can be generated. For example 10."
                        }
                    },
                    "required": ["min", "max"]
                }
            }
        }
    ]
}
```

The `messages` array maps directly to `Vec<Message>`. Each object in the array is one `Message`, serialized by serde. The `tools` array is handled separately by `Context` (as `Vec<serde_json::Value>`), not by `message.rs`.

### Response

```json
{
    "id": "chatcmpl-30",
    "object": "chat.completion",
    "created": 1775285070,
    "model": "qwen3:8b",
    "system_fingerprint": "fp_ollama",
    "choices": [
        {
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_7gz1a7zi",
                        "index": 0,
                        "type": "function",
                        "function": {
                            "name": "random_numbers",
                            "arguments": "{\"min\":1,\"max\":6}"
                        }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        }
    ],
    "usage": {
        "prompt_tokens": 196,
        "completion_tokens": 117,
        "total_tokens": 313
    }
}
```

`async_openai` parses this into `CreateChatCompletionResponse`. The `From` impls in `message.rs` then convert `choices[0].message` into our `Message` struct, extracting `role`, `content`, and `tool_calls`.

---

## `Message`

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}
```

A single `Message` represents one entry in the `messages` array. The four fields cover every message type the API supports:

| Field | When it's used |
|---|---|
| `role` | Always present. Identifies who produced this message. |
| `content` | Always present (may be empty). For user/system messages this is the text. For tool messages this is the tool's result. For assistant messages it's the model's reply (empty when the model only produces tool calls). |
| `tool_calls` | `Some` only in assistant messages where the model wants to invoke tools. Corresponds to the `tool_calls` array in the response JSON above. `None` for every other message type. |
| `tool_call_id` | `Some` only in tool messages. Links this result back to the specific tool call that produced it (the `"id": "call_7gz1a7zi"` from the response). `None` for user, system, and assistant messages. |

Looking at the response JSON: the assistant's message has `"content": ""` and a `tool_calls` array. After the tool runs, the app sends a follow-up message with `"role": "tool"`, `"content": "<result>"`, and `"tool_call_id": "call_7gz1a7zi"` so the model knows which call this result answers.

### Constructors

Three named constructors cover the three message types the app creates (assistant messages come from the API, never constructed manually):

```rust
Message::new_system(content: String) -> Self
```
Sets `role: System`, both optional fields `None`. Used once in `Context::new()` to set the system prompt.

```rust
Message::new_user(content: String) -> Self
```
Sets `role: User`, both optional fields `None`. Created each time the user types a prompt in `lib.rs`.

```rust
Message::new_tool(content: impl ToString, id: String) -> Self
```
Sets `role: Tool`, `tool_call_id: Some(id)`, `tool_calls: None`. The `impl ToString` parameter is a convenience — tool results can be any type that implements `ToString` (an `i32` from the random number generator, a formatted error string, etc.) without the caller needing to convert first.

---

## `Role`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    System,
    Assistant,
    Tool,
}
```

An enum rather than a raw string because:
- The API only accepts these four values. A string would let typos (`"assitant"`) compile silently.
- Match expressions over the enum are exhaustive — the compiler forces you to handle every variant.

`#[serde(rename_all = "lowercase")]` makes `Role::Assistant` serialize to `"assistant"` instead of `"Assistant"`. The API expects lowercase strings, and this attribute handles it at the derive level instead of requiring manual `Serialize`/`Deserialize` impls.

### `From<async_openai::types::chat::Role>`

```rust
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
```

This is a straight 1:1 mapping for the four roles the app uses. `Function` is a deprecated OpenAI role that predates the tool-calling API — it should never appear in responses from modern endpoints, so `unimplemented!()` is appropriate here: if it ever fires, it surfaces as a clear panic rather than silent misbehaviour.

### `Display`

```rust
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
```

Used for CLI output. `Message` also implements `Display` as `"{role}: {content}"`, so printing a message in the terminal shows e.g. `User: roll 1d6` or `Assistant: You rolled a 4!`.

---

## `ToolCall` and `ToolCallFunction`

These two structs model the nested structure inside the response JSON's `tool_calls` array:

```json
{
    "id": "call_7gz1a7zi",
    "type": "function",
    "function": {
        "name": "random_numbers",
        "arguments": "{\"min\":1,\"max\":6}"
    }
}
```

```rust
pub struct ToolCall {
    #[serde(rename = "type")]
    pub tool_call_type: String,
    pub id: String,
    pub function: ToolCallFunction,
}

pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}
```

### `#[serde(rename = "type")]`

The JSON field is called `"type"`, but `type` is a reserved keyword in Rust. `#[serde(rename = "type")]` tells serde to use `"type"` in JSON while the Rust field is named `tool_call_type`.

### Why `arguments` is a `String`

The API returns `arguments` as a JSON-encoded string (`"{\"min\":1,\"max\":6}"`), not as a nested object. Each tool has its own argument schema, so `message.rs` keeps it as a raw `String`. The individual tool modules (like `random_number.rs`) are responsible for deserializing it into their own typed structs:

```rust
// In random_number.rs
let args = serde_json::from_str::<RandomNumberArgs>(&arguments)?;
```

This keeps `message.rs` generic — it doesn't need to know about every tool's parameter shape.

---

## `From` Trait Conversions

Rather than manually pulling fields out of API response types at the call site, `message.rs` implements `From` conversions. This is idiomatic Rust — it centralises the conversion logic and lets you write `Message::from(response_message)` or use `.into()` anywhere.

**Why convert at all?** The API response comes back as `async_openai`'s type: `ChatCompletionResponseMessage`. That's the library's struct — we don't control its shape. We convert it into our own `Message` so the rest of the app only works with one type. Without this, you'd need to handle both `ChatCompletionResponseMessage` (from API) and `Message` (from user/tool) everywhere.

### `Message` from `ChatCompletionResponseMessage`

```rust
impl From<ChatCompletionResponseMessage> for Message {
    fn from(value: ChatCompletionResponseMessage) -> Self {
        let role = Role::from(value.role);
        let content = value.content.unwrap_or_default();
        let tool_calls = value.tool_calls.map(|response_tool_calls| {
            response_tool_calls
                .into_iter()
                .map(ToolCall::from)
                .collect()
        });
        Self {
            role,
            content,
            tool_calls,
            tool_call_id: None,
        }
    }
}
```

- `Role::from(value.role)` converts the library's `async_openai::types::chat::Role` enum into our own `Role` enum. This works because of the [`From<async_openai::types::chat::Role>` impl](#fromasync_openaitypeschatrole) defined further up in the file.
- `content` uses `unwrap_or_default()` because assistant messages with tool calls often have `"content": ""` or `null` — defaulting to an empty string is safe.
- `tool_call_id` is always `None` here because this conversion handles assistant messages from the API, not tool result messages (those are created with `Message::new_tool`).

#### Breaking down the `tool_calls` conversion

This is the most involved part. The API's `tool_calls` field is `Option<Vec<ChatCompletionMessageToolCalls>>` (the library's type). We need to convert it to `Option<Vec<ToolCall>>` (our type). Here's what each layer does:

```rust
// value.tool_calls: Option<Vec<ChatCompletionMessageToolCalls>>

value.tool_calls.map(|response_tool_calls| {   // Option::map — if Some, unwrap the Vec
    response_tool_calls
        .into_iter()                            // iterate over each library tool call
        .map(ToolCall::from)                    // convert each one to OUR ToolCall
        .collect()                              // collect back into Vec<ToolCall>
})

// result: Option<Vec<ToolCall>>
```

There are two `.map()` calls doing different things:
- The **outer** `.map()` is `Option::map` — it handles the `Some`/`None` case. If the API didn't return any tool calls, this stays `None` and the closure never runs.
- The **inner** `.map(ToolCall::from)` is `Iterator::map` — it transforms each element in the `Vec`. `ToolCall::from` is shorthand for `|tc| ToolCall::from(tc)`. This calls the [`From<ChatCompletionMessageToolCalls>` impl](#toolcall-from-chatcompletionmessagetoolcalls) defined below.

**Concrete example** — given this API response:

```json
{
    "role": "assistant",
    "content": "",
    "tool_calls": [
        {
            "id": "call_7gz1a7zi",
            "type": "function",
            "function": { "name": "random_numbers", "arguments": "{\"min\":1,\"max\":6}" }
        }
    ]
}
```

The conversion produces:

```rust
Message {
    role: Role::Assistant,          // Role::from(value.role)
    content: "".to_string(),        // value.content.unwrap_or_default()
    tool_calls: Some(vec![          // Option::map ran because tool_calls was Some
        ToolCall {                  // ToolCall::from converted the library type
            tool_call_type: "function".to_string(),
            id: "call_7gz1a7zi".to_string(),
            function: ToolCallFunction {
                name: "random_numbers".to_string(),
                arguments: "{\"min\":1,\"max\":6}".to_string(),
            },
        },
    ]),
    tool_call_id: None,             // always None for assistant messages
}
```

If the API had returned a simple text response with no tool calls (`"tool_calls": null`), the outer `Option::map` would short-circuit and the result would be `tool_calls: None`.

### `ToolCall` from `ChatCompletionMessageToolCalls`

```rust
impl From<ChatCompletionMessageToolCalls> for ToolCall {
    fn from(value: ChatCompletionMessageToolCalls) -> Self {
        match value {
            ChatCompletionMessageToolCalls::Function(chat_completion_message_tool_call) => {
                let id = chat_completion_message_tool_call.id;
                let tool_call_type = "function".to_owned();
                let function = ToolCallFunction::from(chat_completion_message_tool_call.function);
                Self { tool_call_type, id, function }
            }
            ChatCompletionMessageToolCalls::Custom(_) => unreachable!(),
        }
    }
}
```

`ChatCompletionMessageToolCalls` is an enum from `async_openai`. Currently only the `Function` variant is used in practice. `Custom` is marked `unreachable!()` — it's a catch-all variant in the library for forward compatibility, but no current API endpoint produces it.

### `ToolCallFunction` from `FunctionCall`

```rust
impl From<async_openai::types::assistants::FunctionCall> for ToolCallFunction {
    fn from(value: async_openai::types::assistants::FunctionCall) -> Self {
        let name = value.name;
        let arguments = value.arguments;
        Self { name, arguments }
    }
}
```

A straightforward field-to-field mapping. Extracts `name` (which tool to call) and `arguments` (the raw JSON string of parameters).

---

## How It All Fits Together

Here is the full message lifecycle for a tool-calling interaction. Each step shows the code that runs and the actual API request/response JSON, so you can see how the `messages` array accumulates over time.

### Why `create_byot`?

The app uses `create_byot` ("bring your own types") instead of the standard `async_openai` request builder. To understand why, let's compare both approaches.

**Without `byot` (standard `async_openai`)**, you'd use the library's request types and builder:

```rust
use async_openai::types::chat::{
    CreateChatCompletionRequestArgs,
    ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage,
    ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestToolMessage,
    ChatCompletionToolArgs,
    // ... more types
};

// Before every API call, convert your Messages into the library's request types
let library_messages: Vec<ChatCompletionRequestMessage> = context.messages.iter().map(|msg| {
    match msg.role {
        Role::System => ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessage { content: msg.content.clone(), .. }
        ),
        Role::User => ChatCompletionRequestMessage::User(
            ChatCompletionRequestUserMessage { content: msg.content.clone(), .. }
        ),
        Role::Assistant => ChatCompletionRequestMessage::Assistant(
            ChatCompletionRequestAssistantMessage {
                content: Some(msg.content.clone()),
                tool_calls: /* convert tool_calls too */,
                ..
            }
        ),
        Role::Tool => ChatCompletionRequestMessage::Tool(
            ChatCompletionRequestToolMessage {
                content: msg.content.clone(),
                tool_call_id: msg.tool_call_id.clone().unwrap(),
            }
        ),
    }
}).collect();

// Also convert tools into the library's tool types
let library_tools: Vec<ChatCompletionTool> = /* ... more conversion code ... */;

// Build the request using the library's builder
let request = CreateChatCompletionRequestArgs::default()
    .model("qwen3:8b")
    .messages(library_messages)
    .tools(library_tools)
    .build()?;

let response = client.chat().create(request).await?;
```

This is a lot of conversion boilerplate. Every time you call the API, you'd need to transform your `Message` types into the library's request types, and the library's tools types. You end up maintaining two parallel type systems.

**With `byot`**, none of that is needed. Your `Context` struct already serializes to the exact JSON the API expects (because the field names and serde attributes match the API schema), so you just send it directly:

```rust
let response = client.chat().create_byot(context).await?;
```

The tradeoff is that you're responsible for making sure your types serialize correctly — the library won't validate the request structure for you. But it eliminates the entire conversion layer on the request side.

Note that `ChatCompletionMessageToolCalls` and the other response types from `async_openai` still exist and are still used — `byot` only affects the *request* side. The response is still parsed by the library into `CreateChatCompletionResponse`, and the `From` impls in `message.rs` convert those response types into our `Message`.

A tool-calling interaction requires **two separate round-trips** to the API, not one:

```
Round-trip 1:  App sends [system, user]                          → Model responds with tool call request
               (app executes tool locally — no API call)
Round-trip 2:  App sends [system, user, assistant, tool_result]  → Model responds with "You rolled a 4!"
```

These correspond to `send_to_ai()` at `lib.rs:26` (first call) and `lib.rs:49` (second call after tool execution).

### Who does what?

The model never executes anything — it only *decides* what to call. The app is the middleman that does the actual work and feeds the result back. Here's the full flow:

1. **The user** types a prompt ("roll 1d6"). They don't know tools exist.
2. **The app** sends the prompt to the model, along with the full list of available tools in `Context.tools`. The model has no memory between requests — it needs the tools array every time to know what it can call.
3. **The model** reads the message and the tools list, and *decides* it needs a tool. It responds with the tool name, a call id, and arguments. It does **not** execute anything — this response comes back to the app.
4. **The app** receives that response, matches the tool name, executes the function locally (e.g. `tools::random_number::run()`), and sends the result back to the model in a format it understands (a `tool` role message with the `tool_call_id`).
5. **The model** sees the full history — the user's request, its own tool call, and the tool result — and produces the final assistant content ("You rolled a 4!").
6. **The app** displays that final answer to the user.

### A note on tool selection

In rusty-d20, every tool is included in every request. This is simple and works fine with one tool. But when you have many tools, sending all of them wastes tokens and can confuse the model. An alternative architecture uses an orchestrator that intercepts the request, decides which tools are relevant (using keyword matching, embeddings, or even a smaller LLM), and only attaches those:

```
Current:    User → App → [all tools + messages] → Model
With orchestrator: User → Orchestrator → [selected tools + messages] → Model
```

### Step 1: User sends "roll 1d6"

`Message::new_user("roll 1d6")` is created and added to context (`lib.rs:21-24`). The first API request is sent:

```json
{
    "model": "qwen3:8b",
    "messages": [
        { "role": "system", "content": "You are a friendly AI assistant who uses tools to solve problems" },
        { "role": "user", "content": "roll 1d6" }
    ],
    "tools": [
        {
            "type": "function",
            "function": {
                "name": "random_numbers",
                "description": "Generate a random integer between 'min' and 'max'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "min": { "type": "number", "description": "The lowest possible number that can be generated. For example 0." },
                        "max": { "type": "number", "description": "The largest possible number that can be generated. For example 10." }
                    },
                    "required": ["min", "max"]
                }
            }
        }
    ]
}
```

### Step 2: Model responds with a tool call request

The model sees the tools list and the user's message, and decides it needs the `random_numbers` tool. It does **not** execute anything — it just responds with the tool name, arguments, and a unique call id. The `content` is empty because the model has nothing to say to the user yet:

```json
{
    "choices": [
        {
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_7gz1a7zi",
                        "type": "function",
                        "function": {
                            "name": "random_numbers",
                            "arguments": "{\"min\":1,\"max\":6}"
                        }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        }
    ]
}
```

`Message::from(choice.message)` converts this into our `Message` struct (`lib.rs:75`). The assistant message (with its `tool_calls`) is added to context (`lib.rs:32`).

### Step 3: App executes the tool locally

The model only *requested* a tool call. Now the **app** is responsible for actually running it. The code extracts each tool call, matches by name, and executes the function on your machine (`lib.rs:34-46`):

```rust
let id = tool_call.id;                                          // "call_7gz1a7zi"
let result = tools::random_number::run(tool_call.function.arguments, id);
// returns Message { role: Tool, content: "4", tool_call_id: Some("call_7gz1a7zi") }
context.add_message(result);
```

### Step 4: Context sent back to API with full history

Now the context contains the complete conversation so far. The second API request is sent (`lib.rs:49`):

```json
{
    "model": "qwen3:8b",
    "messages": [
        {
            "role": "system",
            "content": "You are a friendly AI assistant who uses tools to solve problems"
        },
        {
            "role": "user",
            "content": "roll 1d6"
        },
        {
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {
                    "id": "call_7gz1a7zi",
                    "type": "function",
                    "function": {
                        "name": "random_numbers",
                        "arguments": "{\"min\":1,\"max\":6}"
                    }
                }
            ]
        },
        {
            "role": "tool",
            "content": "4",
            "tool_call_id": "call_7gz1a7zi"
        }
    ],
    "tools": [ ... ]
}
```

Notice the `messages` array now has four entries. The `tool_call_id` in the tool message (`"call_7gz1a7zi"`) links back to the `id` in the assistant's `tool_calls` — this is how the model knows which tool call produced which result. Without the tool result message, the model would not know what the tool returned and would behave as if the tool was never called.

### Step 5: API responds with the final answer

The model sees the full history — the user's request, its own tool call, and the tool's result — and produces a natural language answer:

```json
{
    "choices": [
        {
            "message": {
                "role": "assistant",
                "content": "You rolled a 4!"
            },
            "finish_reason": "stop"
        }
    ]
}
```

This is converted via `Message::from()` and added to context (`lib.rs:50-51`). The context now has five messages and is ready for the next user prompt.

### Summary

The `messages` array accumulates the full conversation history. Each round-trip adds messages, and the entire history is re-sent on every API call, giving the model full context of what has happened. The `tool_call_id` field is the critical link that connects a tool result back to the specific tool call that requested it — without it, the model has no way to know what the tool returned.
