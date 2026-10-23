use anyhow::{Context, Result};
use async_trait::async_trait;
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, LlamaModel},
    sampling::LlamaSampler,
};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Mutex;
use crate::domain::ports::processor::TextProcessor;

pub struct LocalProcessor {
    backend: LlamaBackend,
    model: LlamaModel,
    _lock: Mutex<()>,
    system_prompt: String,
}

impl LocalProcessor {
    pub fn load(path: &Path, system_prompt: impl Into<String>) -> Result<Self> {
        let backend = LlamaBackend::init().context("init llama backend")?;
        let model = LlamaModel::load_from_file(&backend, path, &LlamaModelParams::default())
            .context("load model")?;
        Ok(Self { backend, model, _lock: Mutex::new(()), system_prompt: system_prompt.into() })
    }

    fn generate(&self, prompt: &str) -> Result<String> {
        let _guard = self._lock.lock().unwrap();
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(4096));
        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .context("new context")?;

        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), true, true);
        let n_prompt = tokens.len();
        anyhow::ensure!(n_prompt > 0, "empty prompt");

        let mut batch = LlamaBatch::new(n_prompt, 1);
        for (i, &tok) in tokens.iter().enumerate() {
            batch.add(tok, i as i32, &[0], i == n_prompt - 1)
                .context("batch add")?;
        }
        ctx.decode(&mut batch).context("decode prompt")?;

        let eos = vocab.eos();
        let mut sampler = LlamaSampler::greedy();
        let mut out: Vec<u8> = Vec::new();

        for pos in (n_prompt as i32)..(n_prompt as i32 + 1024) {
            let tok = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(tok);
            if tok == eos { break; }
            let piece = vocab.token_to_piece(tok, false, None);
            out.extend_from_slice(&piece);
            batch.clear();
            batch.add(tok, pos, &[0], true).context("batch add loop")?;
            ctx.decode(&mut batch).context("decode loop")?;
        }

        let raw = String::from_utf8_lossy(&out);
        // Strip <think>…</think> block (Qwen3 CoT). If </think> is missing the whole
        // output is a runaway thinking block — return empty so the original transcription is used.
        let text = if let Some(end) = raw.find("</think>") {
            raw[end + "</think>".len()..].trim()
        } else if raw.contains("<think>") {
            ""
        } else {
            raw.trim()
        };
        Ok(text.to_string())
    }
}

#[async_trait]
impl TextProcessor for LocalProcessor {
    async fn process(&self, raw: &str) -> Result<String> {
        // /nothink disables Qwen3 chain-of-thought to save tokens and avoid the <think> block.
        let prompt = format!(
            "<|im_start|>system\n{}<|im_end|>\n\
             <|im_start|>user\n{raw} /nothink<|im_end|>\n\
             <|im_start|>assistant\n",
            self.system_prompt
        );
        tokio::task::block_in_place(|| self.generate(&prompt))
    }
}
