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

The message is cloned before being added to context because we need to inspect `ai_message.tool_calls` on the next line, but `add_message` takes ownership. `.clone()` gives the context its own copy.

#### 3. Tool dispatch (lines 34-52)

```rust
if let Some(tool_calls) = ai_message.tool_calls {
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
```

This is the tool execution pipeline. It only runs when the model's response contains tool calls (`if let Some` — if `tool_calls` is `None`, this entire block is skipped and the loop starts over waiting for the next user prompt).

**The `match` statement** is the tool dispatch table — it maps tool names to their implementations. `tools::random_number::NAME` is the constant `"random_number"` defined in `random_number.rs:5`. When the model requests a tool, the app matches the name and calls the corresponding `run` function.

**The wildcard arm** `_ =>` handles the case where the model requests a tool that doesn't exist. This shouldn't happen if the tools array is correct, but models can hallucinate tool names. Rather than panicking, it sends an error message back as a tool result — the model will see the error and can adjust its response.

**Why `id` matters** — each tool call has a unique `id` (like `"call_7gz1a7zi"`). This id is passed to the tool's `run` function, which includes it in the resulting `Message` as `tool_call_id`. When the context is sent back to the API, the model matches each tool result to the tool call that produced it via this id.

**The `for` loop** handles multiple tool calls in a single response. The model can request several tools at once (e.g., "roll 2d6" might produce two separate `random_number` calls). Each result is added to the context individually.

**The second API call** (`send_to_ai` at line 49) sends the updated context — now containing the original messages plus the assistant's tool call request plus all tool results — back to the model. The model sees the complete history and produces the final natural language answer (e.g., "You rolled a 4!").

### A note on nested tool calls

The current implementation assumes the model won't request more tool calls in its response to tool results. If it did (which is valid in the API — a model can chain tool calls), the second response's `tool_calls` would be silently ignored because there's no recursive handling. For the current single-tool setup this is fine, but a more robust version would use a `while` loop that keeps sending requests until the model responds with `finish_reason: "stop"` instead of `"tool_calls"`.

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

This function handles a single API round-trip. It's called twice during a tool-calling interaction and once for a simple text response.

### `create_byot(context)`

`byot` stands for "bring your own types". Instead of using the library's builder pattern and request types (which would require converting our `Message` structs into the library's `ChatCompletionRequestMessage` types), we pass our `Context` struct directly. Since `Context` derives `Serialize` and its fields match the API's expected JSON structure, this works — `async_openai` serializes it as-is and sends it. See `message.md`'s ["Why `create_byot`?"](message.md#why-create_byot) section for a detailed comparison.

### `.context("Sending message to AI")?`

This is `eyre::Context::context()`, not our `Context` struct. It attaches a human-readable description to the error. If the API call fails (network error, invalid response, etc.), the error will include "Sending message to AI" as context, making it easier to identify which operation failed in the error chain.

### `response.choices[0].clone()`

The API returns a `choices` array, but for non-streaming requests with standard parameters, there's always exactly one choice. Indexing `[0]` directly is fine here — if the array were empty, that would indicate a fundamentally broken API response, and panicking is appropriate.

The `.clone()` is needed because `choices[0]` borrows from `response`, but we want to take ownership of the choice's `message` field to convert it.

### `Message::from(choice.message)`

This calls the `From<ChatCompletionResponseMessage>` impl defined in `message.rs`. It converts the library's response type into our `Message` struct, extracting `role`, `content`, and `tool_calls`. This is the bridge between the library's types and ours — after this point, the rest of the app only works with `Message`.
