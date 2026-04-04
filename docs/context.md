# context.rs

`context.rs` defines the `Context` struct — the single object that gets serialized into the JSON body of every API request. It holds the model name, the full conversation history, and the list of available tools. Because `Context` is sent directly to the API via `create_byot`, its field names and structure must match the OpenAI chat completions request schema exactly.

---

## The API Contract

Every call to the chat completions endpoint requires a JSON body with three top-level fields: `model`, `messages`, and `tools`. `Context` maps 1:1 to this shape:

```json
{
    "model": "qwen3:8b",
    "messages": [ ... ],
    "tools": [ ... ]
}
```

Because `Context` derives `Serialize`, and its field names already match the API's expected keys, `create_byot` can serialize it directly — no intermediate conversion types needed. This is the key design decision explained in detail in `message.md` under [Why `create_byot`?](message.md#why-create_byot).

---

## `Context`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Value>,
}
```

| Field | Type | Purpose |
|---|---|---|
| `model` | `String` | The model identifier sent in every request (e.g. `"qwen3:8b"`). Set once at construction and never changes. |
| `messages` | `Vec<Message>` | The full conversation history. Grows over the lifetime of a session — every user prompt, assistant reply, and tool result is appended here. Re-sent in its entirety on every API call because the model has no memory between requests. |
| `tools` | `Vec<Value>` | The list of tools the model is allowed to call. Each entry is a raw `serde_json::Value` matching the tool object schema from the API. Also re-sent on every request. |

### Why `Vec<Value>` for tools?

Each tool definition is a nested JSON object with a specific schema (`type`, `function.name`, `function.description`, `function.parameters`). Rather than creating Rust structs that mirror this schema and derive `Serialize`, the app builds each tool definition as a `serde_json::Value` using the `json!()` macro (see `random_number.rs:7-35`). This is simpler for a small number of tools — there's no struct to maintain, and the shape is immediately visible as literal JSON.

The tradeoff is the same as with `create_byot`: the compiler won't catch a malformed tool definition. If you misspell `"parameters"` as `"paramters"` inside a `json!()` block, it compiles fine but the API will ignore the tool or return an error at runtime.

---

## `Context::new`

```rust
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
```

This is the only place where the system prompt and the tool registry are defined. When `Context::new` is called (at `lib.rs:11`), it:

1. **Creates the system message** — `Message::new_system(...)` produces a `Message` with `role: System` and the given content. This is always the first entry in `messages`, so it appears first in every API request. The system prompt tells the model its persona and that it should use tools.

2. **Registers available tools** — `tools::random_number::create_tool()` returns a `serde_json::Value` containing the tool's JSON definition (the same structure shown in the request JSON in `message.md`). All tools are collected into a `Vec` and stored in `self.tools`.

3. **Stores the model name** — passed in from `main.rs`, where it's read from the `AI_MODEL` environment variable (defaulting to `"llama3.2:1b"`).

### Adding a new tool

To register a new tool, you would:
1. Create a new module under `tools/` with a `create_tool()` function that returns a `Value`.
2. Add it to the `available_tools` vec here: `vec![tools::random_number::create_tool(), tools::new_tool::create_tool()]`.
3. Add a match arm in the tool dispatch in `lib.rs:39-44` so the app knows how to execute it.

---

## `Context::add_message`

```rust
pub fn add_message(&mut self, message: Message) {
    self.messages.push(message)
}
```

Appends a message to the conversation history. This is called throughout `lib.rs` at each stage of the conversation:

- After the user types a prompt (`lib.rs:24`)
- After the model responds (`lib.rs:32`)
- After a tool produces a result (`lib.rs:46`)
- After the model responds to the tool result (`lib.rs:51`)

Because the entire `messages` vec is serialized and sent on every API call, the order matters. The API expects messages in chronological order, and specifically expects a `tool` message to appear after the `assistant` message that requested the tool call.

---

## How Context Grows Over a Conversation

Here's what `context.messages` looks like at each stage of a tool-calling interaction (tool definitions in `context.tools` stay constant):

**After construction:**
```
[system]
```

**After user types "roll 1d6":**
```
[system, user]
```

**After first API response (model requests tool):**
```
[system, user, assistant(tool_calls)]
```

**After tool execution:**
```
[system, user, assistant(tool_calls), tool(result)]
```

**After second API response (model gives final answer):**
```
[system, user, assistant(tool_calls), tool(result), assistant("You rolled a 4!")]
```

**After next user prompt "roll again":**
```
[system, user, assistant(tool_calls), tool(result), assistant, user]
```

The history never shrinks. Every message ever sent or received stays in the vec and is re-sent to the API on the next call. This gives the model full conversational context — it can refer back to previous rolls, previous questions, etc. The downside is that token usage grows with every exchange. For a long-running session, you'd eventually hit the model's context window limit. Strategies like summarisation or sliding-window truncation could address this, but rusty-d20 doesn't implement them.
