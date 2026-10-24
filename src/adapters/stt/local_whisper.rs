use async_trait::async_trait;
use anyhow::{bail, Context, Result};
use hound::{WavWriter, WavSpec, SampleFormat};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::io::Cursor;
use crate::domain::model::{AudioSamples, Transcription};
use crate::domain::ports::stt::SpeechToText;
use super::shared_vocab::SharedVocab;

pub struct LocalWhisper {
    python: String,
    server_script: String,
    model: String,
    device: String,
    lang: String,
    prompt: SharedVocab,
    state: Mutex<Option<ServerState>>,
}

struct ServerState {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl LocalWhisper {
    pub fn new(
        python: impl Into<String>,
        server_script: impl Into<String>,
        model: impl Into<String>,
        device: impl Into<String>,
        lang: impl Into<String>,
        prompt: SharedVocab,
    ) -> Self {
        Self {
            python: python.into(),
            server_script: server_script.into(),
            model: model.into(),
            device: device.into(),
            lang: lang.into(),
            prompt,
            state: Mutex::new(None),
        }
    }

    /// Detect the best available Python binary and server script.
    pub fn detect(model: impl Into<String>, lang: impl Into<String>, prompt: SharedVocab) -> Option<Self> {
        let model = model.into();
        let lang = lang.into();

        // Server script embedded at compile time, written to data dir on first use.
        let python_candidates = [
            "python3",
            "python",
        ];
        for python in python_candidates {
            if std::path::Path::new(python).exists() || which(python) {
                // Check whisper is importable
                let ok = Command::new(python)
                    .args(["-c", "import whisper"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if ok {
                    let script = server_script_path();
                    let device = if Command::new("nvidia-smi").output()
                        .map(|o| o.status.success()).unwrap_or(false) { "cuda" } else { "cpu" };
                    return Some(Self::new(python, script, model, device, lang, prompt.clone()));
                }
            }
        }
        None
    }

    /// Ensure the server is running; start it if needed.
    fn ensure_server(&self) -> Result<()> {
        let mut guard = self.state.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
        // Write server script to disk if not present.
        ensure_server_script(&self.server_script)?;

        tracing::info!("starting whisper server ({} on {})…", self.model, self.device);
        let mut child = Command::new(&self.python)
            .args([&self.server_script, &self.model, &self.device])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn whisper server")?;

        let stdin = child.stdin.take().context("whisper server stdin")?;
        let raw_stdout = child.stdout.take().context("whisper server stdout")?;
        let mut stdout = BufReader::new(raw_stdout);

        // Wait for {"ready": true}
        let mut line = String::new();
        stdout.read_line(&mut line).context("whisper server ready")?;
        let v: serde_json::Value = serde_json::from_str(line.trim())
            .context("whisper server ready json")?;
        if v["ready"] != true {
            bail!("whisper server did not send ready: {line}");
        }
        tracing::info!("whisper server ready");
        *guard = Some(ServerState { child, stdin, stdout });
        Ok(())
    }

    fn encode_wav(&self, audio: &AudioSamples) -> Result<Vec<u8>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate: audio.sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut buf = Cursor::new(Vec::new());
        let mut writer = WavWriter::new(&mut buf, spec)?;
        let step = audio.channels as usize;
        for chunk in audio.samples.chunks(step) {
            let mono = chunk.iter().sum::<f32>() / step as f32;
            writer.write_sample((mono.clamp(-1.0, 1.0) * 32767.0) as i16)?;
        }
        writer.finalize()?;
        Ok(buf.into_inner())
    }
}

fn server_script_path() -> String {
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("whisper_server.py").to_string_lossy().into_owned()
}

fn ensure_server_script(path: &str) -> Result<()> {
    if !std::path::Path::new(path).exists() {
        std::fs::write(path, include_str!("../../../assets/whisper_server.py"))
            .context("write whisper_server.py")?;
    }
    Ok(())
}

fn which(bin: &str) -> bool {
    Command::new("which").arg(bin).output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[async_trait]
impl SpeechToText for LocalWhisper {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        self.ensure_server()?;

        // Use ramdisk when available — avoids disk I/O for the temp WAV (typically 300-600 KB).
        let tmp_base = if std::path::Path::new("/dev/shm").exists() {
            std::path::PathBuf::from("/dev/shm")
        } else {
            std::env::temp_dir()
        };
        let dir = tempfile::Builder::new().tempdir_in(&tmp_base).context("tmpdir")?;
        let wav_path = dir.path().join("audio.wav");
        let wav = self.encode_wav(audio)?;
        std::fs::write(&wav_path, &wav).context("write wav")?;

        let mut guard = self.state.lock().unwrap();
        let state = guard.as_mut().context("whisper server not running")?;

        let prompt = self.prompt.lock().unwrap().clone();
        let line = format!("{}\t{}\t{}\n",
            wav_path.to_str().unwrap(),
            self.lang,
            prompt,
        );
        state.stdin.write_all(line.as_bytes()).context("send to whisper")?;
        state.stdin.flush().context("flush whisper stdin")?;

        let mut resp = String::new();
        state.stdout.read_line(&mut resp).context("read whisper response")?;

        let v: serde_json::Value = serde_json::from_str(resp.trim())
            .context("parse whisper response")?;
        if let Some(e) = v["error"].as_str() {
            bail!("whisper error: {e}");
        }
        // Reject hallucinations: Whisper marks silence/noise with high no_speech_prob.
        let no_speech = v["no_speech_prob"].as_f64().unwrap_or(0.0);
        if no_speech > 0.6 {
            tracing::debug!("whisper: no_speech_prob={no_speech:.2} — discarding as silence/noise");
            return Ok(Transcription { text: String::new() });
        }
        let text = v["text"].as_str().unwrap_or("").trim().to_string();
        Ok(Transcription { text })
    }
}

impl Drop for LocalWhisper {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(mut s) = guard.take() {
                let _ = s.child.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_response(json: &str) -> Result<Transcription> {
        let v: serde_json::Value = serde_json::from_str(json).context("parse")?;
        if let Some(e) = v["error"].as_str() {
            bail!("whisper error: {e}");
        }
        let no_speech = v["no_speech_prob"].as_f64().unwrap_or(0.0);
        if no_speech > 0.6 {
            return Ok(Transcription { text: String::new() });
        }
        let text = v["text"].as_str().unwrap_or("").trim().to_string();
        Ok(Transcription { text })
    }

    #[test]
    fn low_no_speech_prob_keeps_text() {
        let r = parse_response(r#"{"text": "bonjour", "no_speech_prob": 0.1}"#).unwrap();
        assert_eq!(r.text, "bonjour");
    }

    #[test]
    fn high_no_speech_prob_returns_empty() {
        let r = parse_response(r#"{"text": "hallucination garbage", "no_speech_prob": 0.85}"#).unwrap();
        assert!(r.text.is_empty(), "should discard hallucination");
    }

    #[test]
    fn exactly_at_threshold_keeps_text() {
        let r = parse_response(r#"{"text": "borderline", "no_speech_prob": 0.6}"#).unwrap();
        assert_eq!(r.text, "borderline"); // > 0.6 required to discard
    }

    #[test]
    fn missing_no_speech_prob_keeps_text() {
        let r = parse_response(r#"{"text": "hello"}"#).unwrap();
        assert_eq!(r.text, "hello");
    }

    #[test]
    fn error_field_propagates() {
        let r = parse_response(r#"{"error": "file not found"}"#);
        assert!(r.is_err());
    }
}
