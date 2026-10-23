pub mod ollama_processor;
pub mod openai_processor;
pub mod anthropic_processor;
pub mod local_processor;
pub use ollama_processor::OllamaProcessor;
pub use openai_processor::OpenAiProcessor;
pub use anthropic_processor::AnthropicProcessor;
pub use local_processor::LocalProcessor;
