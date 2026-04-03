# Rusty-D20
An autonomous Rust agent whose only directive in life is to roll the dice for you

## How LLMs Interact with Tools via the OpenAI API Standard

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
