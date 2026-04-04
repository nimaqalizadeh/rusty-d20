# lib.rs

`lib.rs` is the application's main loop — the orchestrator that ties together the user's terminal input, the AI model, and the tool system. It manages the conversation lifecycle: read a prompt, send it to the model, check if the model wants to call a tool, execute the tool if so, send the result back, and display the final answer. Everything in `message.rs` and `context.rs` exists to support what happens here.

---

## Module Declarations

```rust
mod context;
mod message;
mod tools;
```

These three `mod` declarations make the submodules available within the library crate. They're private — nothing outside this crate can reach `context`, `message`, or `tools` directly. The only public entry point is the `run` function.

---

## Imports and the Name Collision

```rust
use crate::{context::Context as AIContext, message::Message};
use async_openai::{Client, config::OpenAIConfig, types::chat::CreateChatCompletionResponse};
use eyre::{Context, Result};
```

There's a name collision here: both `crate::context::Context` (our struct) and `eyre::Context` (the error-handling trait) are called `Context`. The import resolves this by renaming ours to `AIContext`:

```rust
use crate::context::Context as AIContext;
```

`eyre::Context` is a trait that adds the `.context("...")` method to `Result` types, used at `lib.rs:71` to attach a human-readable message to API errors. Without the rename, the compiler wouldn't know which `Context` you meant.

---

## `run`

```rust
pub async fn run(model: String, api_key: String, api_base: String) -> Result<()>
```

This is the only public function in the crate. `main.rs` calls it after reading environment variables. It's `async` because the API calls are asynchronous (they go over the network and we `await` the response rather than blocking the thread).

### Setup (lines 11-17)

```rust
let mut context = AIContext::new(model);

let config = OpenAIConfig::new()
    .with_api_key(api_key)
    .with_api_base(api_base);

let client = Client::with_config(config);
```

Two things are initialised once and reused for the entire session:

1. **`context`** — our `Context` struct. Created with the model name, it starts with the system prompt and tool definitions already inside. It's `mut` because messages will be pushed into it throughout the conversation.

2. **`client`** — the `async_openai` HTTP client. `OpenAIConfig` points it at the right base URL (Ollama at `localhost:11434/v1` by default) and attaches the API key. The `Client` handles connection pooling, serialisation, and HTTP transport — we just call `.chat().create_byot(context)` on it.

### The Main Loop (lines 19-53)

```rust
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

    if let Some(tool_calls) = ai_message.tool_calls {
        // ... tool handling ...
    }
}
```

The `loop` runs forever — there's no quit command, you exit with Ctrl+C. Each iteration is one conversational turn. Here's what happens step by step:

#### 1. Get user input (lines 20-24)

```rust
let user_prompt = get_user_prompt()?;
let user_message = Message::new_user(user_prompt);
println!("{user_message}");
context.add_message(user_message);
```

`get_user_prompt()` blocks on stdin, waiting for the user to type something and press Enter. The input is wrapped in a `Message` with `role: User`, printed to the terminal (showing `User: roll 1d6` via the `Display` impl), and added to the context's message history.

#### 2. First API call (lines 26-32)

```rust
let ai_message = send_to_ai(&context, &client).await?;

if !ai_message.content.is_empty() {
    println!("{ai_message}");
}

context.add_message(ai_message.clone());
```

The entire context (system prompt + all previous messages + new user message) is sent to the model. The response is converted into our `Message` type via the `From` impl in `message.rs`.

The `if !ai_message.content.is_empty()` check exists because when the model decides to call a tool, it often returns an empty `content` field — there's nothing to say to the user yet. Printing an empty message would show `Assistant: ` with nothing after it, so we skip it.

The message is cloned before being added to context because `add_message` takes ownership. `.clone()` gives the context its own copy. After this line, the message is inside `context.messages` — the `while` loop on line 34 reads it back from there to check for tool calls.

#### 3. Tool-calling loop (lines 34-58)

```rust
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
```

This is the tool execution pipeline. It uses a `while` loop instead of a one-shot `if`, which means the app can handle **chained tool calls** — when the model's response to a tool result requests *another* tool call, the loop runs again. It keeps going until the last message in the context has no tool calls, meaning the model has produced a final text answer.

**The `while` condition** checks whether the most recent message in the conversation has tool calls:

```rust
context.messages.last().is_some_and(|message| {
    message.tool_calls.clone().is_some_and(|tool_calls| !tool_calls.is_empty())
})
```

This reads as: "if there is a last message, and it has `tool_calls`, and those tool calls are not empty, keep looping." On the first iteration, the last message is the assistant response from line 32. On subsequent iterations, it's the `ai_tool_response` added at line 56. When the model finally responds with text only (no tool calls), the condition is false and the loop exits.

The `.clone()` on `tool_calls` is needed because `is_some_and` takes ownership of the value inside the `Option`, but we're borrowing the message from `context.messages` — we can't move out of a borrow. Cloning the `Option<Vec<ToolCall>>` lets `is_some_and` consume the clone while leaving the original intact.

**The inner `if let`** might look redundant after the `while` condition, but it serves a different purpose. The `while` condition borrows the last message immutably to check whether tool calls exist. The `if let` then gets an owned copy of the tool calls (via `.cloned().unwrap()`) so we can iterate over them while also mutating `context` (calling `context.add_message`). We can't hold an immutable borrow of `context.messages` and call `&mut self` methods on `context` at the same time — Rust's borrow checker prevents this. Getting a cloned copy first releases the borrow.

**The `match` statement** is the tool dispatch table — it maps tool names to their implementations. `tools::random_number::NAME` is the constant `"random_number"` defined in `random_number.rs:5`. When the model requests a tool, the app matches the name and calls the corresponding `run` function.

**The wildcard arm** `_ =>` handles the case where the model requests a tool that doesn't exist. This shouldn't happen if the tools array is correct, but models can hallucinate tool names. Rather than panicking, it sends an error message back as a tool result — the model will see the error and can adjust its response.

**Why `id` matters** — each tool call has a unique `id` (like `"call_7gz1a7zi"`). This id is passed to the tool's `run` function, which includes it in the resulting `Message` as `tool_call_id`. When the context is sent back to the API, the model matches each tool result to the tool call that produced it via this id.

**The `for` loop** handles multiple tool calls in a single response. The model can request several tools at once (e.g., "roll 2d6" might produce two separate `random_number` calls). Each result is added to the context individually.

**The API call inside the loop** (`send_to_ai` at line 54) sends the updated context — now containing the original messages plus the assistant's tool call request plus all tool results — back to the model. If the model responds with more tool calls, the `while` condition is true again and the loop runs another iteration. If the model responds with a final text answer, the loop exits.

### Example: chained tool calls

Imagine the model decides it needs two separate pieces of information to answer a question. With the `while` loop, the flow looks like:

```
Iteration 1:  last message has tool_calls → execute tool → send results → model responds with MORE tool_calls
Iteration 2:  last message has tool_calls → execute tool → send results → model responds with text only
Loop exits:   last message has no tool_calls
```

Without the loop (with a one-shot `if`), the second set of tool calls would be silently ignored and the model's answer would be incomplete.

---

## `get_user_prompt`

```rust
pub fn get_user_prompt() -> Result<String> {
    let mut prompt = String::new();

    print!("> ");
    stdout().flush()?;
    stdin().read_line(&mut prompt)?;

    Ok(prompt.trim().to_owned())
}
```

A simple terminal prompt. The details:

- **`print!("> ")`** uses `print!` not `println!` — no newline, so the cursor stays on the same line as the prompt character.
- **`stdout().flush()?`** is necessary because `print!` doesn't flush automatically (unlike `println!`). Without this, the `> ` might not appear on screen before `read_line` blocks waiting for input.
- **`stdin().read_line(&mut prompt)?`** blocks until the user presses Enter. The input includes the trailing newline.
- **`prompt.trim().to_owned()`** removes the trailing newline (and any leading/trailing whitespace) and converts the `&str` back to an owned `String`.

---

## `send_to_ai`

```rust
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
```

This function handles a single API round-trip. It's called once for a simple text response, twice for a single tool-calling interaction, and potentially more times if the model chains multiple rounds of tool calls.

### `create_byot(context)`

`byot` stands for "bring your own types". Instead of using the library's builder pattern and request types (which would require converting our `Message` structs into the library's `ChatCompletionRequestMessage` types), we pass our `Context` struct directly. Since `Context` derives `Serialize` and its fields match the API's expected JSON structure, this works — `async_openai` serializes it as-is and sends it. See `message.md`'s ["Why `create_byot`?"](message.md#why-create_byot) section for a detailed comparison.

### `.context("Sending message to AI")?`

This is `eyre::Context::context()`, not our `Context` struct. It attaches a human-readable description to the error. If the API call fails (network error, invalid response, etc.), the error will include "Sending message to AI" as context, making it easier to identify which operation failed in the error chain.

### `response.choices[0].clone()`

The API returns a `choices` array, but for non-streaming requests with standard parameters, there's always exactly one choice. Indexing `[0]` directly is fine here — if the array were empty, that would indicate a fundamentally broken API response, and panicking is appropriate.

The `.clone()` is needed because `choices[0]` borrows from `response`, but we want to take ownership of the choice's `message` field to convert it.

### `Message::from(choice.message)`

This calls the `From<ChatCompletionResponseMessage>` impl defined in `message.rs`. It converts the library's response type into our `Message` struct, extracting `role`, `content`, and `tool_calls`. This is the bridge between the library's types and ours — after this point, the rest of the app only works with `Message`.
