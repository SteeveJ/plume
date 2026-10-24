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
use std::sync::{Arc, Mutex};
use crate::domain::ports::processor::TextProcessor;

// Budget de génération fixe (Qwen3 réfléchit avant de répondre : le budget couvre la réflexion et la réponse) ; KV cache = prompt + budget au lieu de 4096 fixes (~450 Mo).
const MAX_NEW: u32 = 1024;
fn ctx_sizes(n_prompt: usize) -> (u32, u32) {
    (n_prompt as u32 + MAX_NEW, MAX_NEW)
}

struct Inner {
    backend: LlamaBackend,
    model: LlamaModel,
    _lock: Mutex<()>,
    system_prompt: String,
}

pub struct LocalProcessor {
    inner: Arc<Inner>,
}

impl LocalProcessor {
    pub fn load(path: &Path, system_prompt: impl Into<String>) -> Result<Self> {
        let backend = LlamaBackend::init().context("init llama backend")?;
        let model = LlamaModel::load_from_file(&backend, path, &LlamaModelParams::default())
            .context("load model")?;
        Ok(Self { inner: Arc::new(Inner { backend, model, _lock: Mutex::new(()), system_prompt: system_prompt.into() }) })
    }
}

impl Inner {
    fn generate(&self, prompt: &str) -> Result<String> {
        let _guard = self._lock.lock().unwrap();
        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), true, true);
        let n_prompt = tokens.len();
        anyhow::ensure!(n_prompt > 0, "empty prompt");

        let (n_ctx, max_new) = ctx_sizes(n_prompt);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_n_batch(n_prompt as u32);
        // shortcut: context recreated per call (~100ms); LlamaContext<'model> borrows self prevents caching
        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .context("new context")?;

        let mut batch = LlamaBatch::new(n_prompt, 1);
        for (i, &tok) in tokens.iter().enumerate() {
            batch.add(tok, i as i32, &[0], i == n_prompt - 1)
                .context("batch add")?;
        }
        ctx.decode(&mut batch).context("decode prompt")?;

        let eos = vocab.eos();
        let mut sampler = LlamaSampler::greedy();
        let mut out: Vec<u8> = Vec::new();

        for pos in (n_prompt as i32)..(n_prompt as i32 + max_new as i32) {
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

/// Gabarit Qwen3 : la réflexion reste activée, car le petit modèle corrige bien mieux ainsi (mesuré : en mode sans réflexion
/// il rend le plus souvent le texte inchangé). Le bloc `<think>` est retiré de la sortie. Aucun suffixe `/nothink` :
/// Qwen3 ne le reconnaît pas (le vrai est `/no_think`) et le modèle le recopiait dans le texte corrigé.
fn chat_prompt(system: &str, raw: &str) -> String {
    format!("<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{raw}<|im_end|>\n<|im_start|>assistant\n")
}

#[async_trait]
impl TextProcessor for LocalProcessor {
    async fn process(&self, raw: &str) -> Result<String> {
        let prompt = chat_prompt(&self.inner.system_prompt, raw);
        // Hors du thread async : la boucle de dictée reste libre de voir une nouvelle pression de touche.
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.generate(&prompt)).await.context("llm task")?
    }
}

#[cfg(test)]
mod tests {
    use super::{chat_prompt, ctx_sizes};

    #[test]
    fn prompt_does_not_pollute_the_user_text() {
        let p = chat_prompt("SYS", "bonjour");
        assert!(p.starts_with("<|im_start|>system\nSYS<|im_end|>"));
        assert!(p.contains("<|im_start|>user\nbonjour<|im_end|>"));
        assert!(p.ends_with("<|im_start|>assistant\n"));
        assert!(!p.contains("nothink") && !p.contains("no_think"));
    }

    #[test]
    fn ctx_scales_with_prompt_and_stays_bounded() {
        assert_eq!(ctx_sizes(100), (1124, 1024));
        assert_eq!(ctx_sizes(3000), (4024, 1024));
    }
}
