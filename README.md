# Rusty-D20

A minimal AI agent built in Rust to explore how tool-calling agents actually work under the hood.

## What Is This?

This project was an experiment to understand the mechanics of AI agents — specifically, how an LLM can go beyond generating text and actually *do things* by calling tools in a loop.

The agent itself is simple: it's a CLI chatbot that can roll dice (generate random numbers) when you ask it to. But the interesting part isn't what it does — it's *how* it does it. The agent loop reads user input, sends it to an LLM, checks if the model wants to call a tool, executes that tool, feeds the result back to the model, and repeats until the model is done.

## How It Works

```
User prompt → LLM → (tool call?) → execute tool → feed result back → LLM → response
                ↑                                                        |
                └────────────────────────────────────────────────────────┘
```

1. The user types a prompt (e.g., "roll a d20")
2. The prompt is sent to the LLM along with a list of available tools
3. If the model decides it needs a tool, it returns a `tool_calls` response instead of plain text
4. The agent executes the requested tool (e.g., generates a random number between 1 and 20)
5. The tool result is sent back to the model as a `tool` message
6. The model uses the result to produce its final response
7. Steps 3–6 repeat if the model wants to call more tools (chained tool calls)

## Project Structure

- **`src/main.rs`** — Entry point, loads config from environment variables
- **`src/lib.rs`** — The core agent loop: prompt → send → tool call → repeat
- **`src/context.rs`** — Manages the conversation history and tool definitions sent to the LLM
- **`src/message.rs`** — Message types (user, assistant, tool) and serialization
- **`src/tools/`** — Tool implementations (currently just `random_number`)

## Running It

The agent uses the OpenAI-compatible API format, so it works with any provider that supports it (Ollama, OpenAI, etc.).

```bash
# With Ollama (default)
cargo run

# With a different provider
API_KEY=your-key BASE_URL=https://api.openai.com/v1 AI_MODEL=gpt-4o cargo run
```

## What I Learned

The key takeaway is that an "agent" is really just a loop around an LLM call. The model doesn't execute anything — it outputs structured text saying *what* it wants to call, and the application code handles the actual execution. The OpenAI API standard makes this portable across providers, so the same Rust code works whether you're hitting a local Ollama instance or a cloud API.

---

## Deep Dive: How LLMs Interact with Tools via the OpenAI API Standard

### The Model Is Just a Text Predictor

An LLM is a neural network that takes in tokens (text) and predicts the next token. It has no native understanding of JSON, API fields, or tool definitions. It only ever sees and produces text.

### The Role of the API Layer

Between your application code and the model sits an **API layer** that handles translation in both directions:

```
Application (JSON) → API Layer → Formatted text prompt → Model → Raw text output → API Layer → Structured JSON response
```

- **Inbound**: The API layer takes structured fields like `"tools"` from your request and converts them into a text format the model was fine-tuned to recognize (e.g., special tokens or a system prompt describing available functions).
- **Outbound**: When the model outputs text matching a tool-call pattern, the API layer parses it back into a structured `tool_calls` JSON response.

This means the API layer acts as a strict gatekeeper. It only processes fields with **exact expected names** from the specification. For example, sending `"tool"` (singular) instead of `"tools"` (plural) causes the field to be silently dropped — the model never receives any tool information, even though the difference is a single character.

### Before the OpenAI Standard

Before OpenAI's API format became the de facto standard, each model provider (Anthropic, Cohere, Google, etc.) had its own API with different request/response structures and field names. Switching between models required rewriting integration code entirely. This fragmentation drove the popularity of abstraction libraries like LangChain, which provided a unified interface over multiple providers.

### The Standard Today

Most providers now expose OpenAI-compatible endpoints (e.g., Ollama serves `/v1/chat/completions`), allowing the same client library (like `async-openai` in Rust) to communicate with entirely different underlying models. This reduces the need for heavy abstraction frameworks — for most use cases (chat, tool calling, structured output), the standard API is sufficient on its own.
