pub mod openai_whisper;
pub mod local_whisper;
pub mod shared_vocab;
pub use openai_whisper::OpenAiWhisper;
pub use local_whisper::LocalWhisper;
pub use shared_vocab::{SharedVocab, new_shared_vocab};
