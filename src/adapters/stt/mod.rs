pub mod openai_whisper;
pub mod whisper_cpp;
pub mod shared_vocab;
pub mod audio_util;
#[cfg(feature = "opus")]
pub mod opus_enc;
pub use openai_whisper::OpenAiWhisper;
pub use whisper_cpp::WhisperCpp;
pub use shared_vocab::{SharedVocab, new_shared_vocab};
