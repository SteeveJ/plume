//! Non-regression: complete dictation chain with the real adapters (SQLite history, scratchpad, snippets)
//! and fakes for the hardware (microphone, virtual keyboard) and the models.
mod common;
use anyhow::Result;
use async_trait::async_trait;
use common::{dictate, voice, FakeMic, FakeStt, Notes, Typed};
use plume::adapters::{history::SqliteHistory, scratchpad::FileScratchpad};
use plume::application::{dictation::DictationService, snippets};
use plume::config::OutputMode;
use plume::domain::ports::processor::TextProcessor;

struct Capitalize;
#[async_trait]
impl TextProcessor for Capitalize {
    async fn process(&self, raw: &str) -> Result<String> {
        let mut c = raw.chars();
        Ok(c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str() + ".").unwrap_or_default())
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plume-it-{name}-{}", std::process::id()))
}

#[test]
fn text_flows_from_speech_to_processor_snippets_history_and_keyboard() {
    let (db, pad) = (temp("hist.db"), temp("scratch.txt"));
    let _ = (std::fs::remove_file(&db), std::fs::remove_file(&pad));
    let typed = Typed::default();
    let mut svc = DictationService::new(FakeMic::one(voice()), FakeStt::new("mon email est bon").0, typed.clone(),
            Some(Box::new(Capitalize)), Box::new(Notes::default()))
        .with_history(Box::new(SqliteHistory::open_at(&db).unwrap()))
        .with_scratchpad(Box::new(FileScratchpad::new(pad.clone())))
        .with_output(OutputMode::Both)
        .with_snippets(snippets::parse("mon email => jean@example.com\n"));
    dictate(&mut svc, 1);

    // The snippet acts after the LLM (otherwise the LLM could distort the address), case-insensitive.
    assert_eq!(*typed.0.lock().unwrap(), vec!["jean@example.com est bon.".to_string()]);
    let conn = rusqlite::Connection::open(&db).unwrap();
    let (raw, text): (String, String) = conn.query_row("SELECT raw, text FROM history", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((raw.as_str(), text.as_str()), ("mon email est bon", "jean@example.com est bon."));
    assert!(std::fs::read_to_string(&pad).unwrap().contains("jean@example.com est bon."));
    let _ = (std::fs::remove_file(&db), std::fs::remove_file(&pad));
}

#[test]
fn silence_types_nothing_and_stores_nothing() {
    let db = temp("silence.db");
    let _ = std::fs::remove_file(&db);
    let typed = Typed::default();
    let mut svc = DictationService::new(FakeMic::one(common::muted()), FakeStt::new("ne doit jamais sortir").0, typed.clone(), None, Box::new(Notes::default()))
        .with_history(Box::new(SqliteHistory::open_at(&db).unwrap()));
    dictate(&mut svc, 1);
    let rows: i64 = rusqlite::Connection::open(&db).unwrap().query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0)).unwrap();
    let _ = std::fs::remove_file(&db);
    assert!(typed.0.lock().unwrap().is_empty());
    assert_eq!(rows, 0);
}

#[test]
fn a_processor_that_loses_words_never_replaces_the_dictation() {
    struct Lossy;
    #[async_trait]
    impl TextProcessor for Lossy {
        async fn process(&self, _: &str) -> Result<String> { Ok("Résumé.".into()) }
    }
    let raw = "alors je pense que nous devrions nous voir lundi matin avec marie pour parler du budget des ventes";
    let typed = Typed::default();
    let mut svc = DictationService::new(FakeMic::one(voice()), FakeStt::new(raw).0, typed.clone(), Some(Box::new(Lossy)), Box::new(Notes::default()));
    dictate(&mut svc, 1);
    assert_eq!(*typed.0.lock().unwrap(), vec![raw.to_string()]);
}
