//! Instruction mode: select a text, hold the second key, say what to do with it, the result replaces the selection.
//! Real adapters (SQLite history, scratchpad), fake microphone, keyboard, Whisper and online model.
mod common;
use common::*;
use plume::adapters::{history::SqliteHistory, scratchpad::FileScratchpad};
use plume::application::dictation::DictationService;
use plume::config::OutputMode;
use plume::domain::model::HotkeyEvent::{InstructStart, InstructStop, RecordStart, RecordStop};
use std::sync::atomic::Ordering;

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plume-instruct-{name}-{}", std::process::id()))
}

#[test]
fn the_spoken_instruction_rewrites_the_selection_and_is_kept_in_the_history() {
    let (db, pad) = (temp("hist.db"), temp("scratch.txt"));
    let _ = (std::fs::remove_file(&db), std::fs::remove_file(&pad));
    let (stt, _) = FakeStt::confirmed("mets en majuscules");
    let (typed, notes, calls) = (Typed::default(), Notes::default(), Default::default());
    let mut svc = DictationService::new(FakeMic::one(voice()), stt, typed.clone(), None, Box::new(notes.clone()))
        .with_history(Box::new(SqliteHistory::open_at(&db).unwrap()))
        .with_scratchpad(Box::new(FileScratchpad::new(pad.clone())))
        .with_output(OutputMode::Scratchpad)
        .with_instruct(Box::new(Selected(Some("le chat dort".into()))), Box::new(FakeRewriter(std::sync::Arc::clone(&calls))));
    press(&mut svc, vec![InstructStart, InstructStop]);

    // The result is pasted over the selection, even if normal dictation is configured as "scratchpad".
    assert_eq!(typed.texts(), vec!["LE CHAT DORT".to_string()], "{}", notes.all());
    assert!(notes.all().contains("Rewriting") && notes.all().contains("Ctrl+Z"), "{}", notes.all());
    assert!(!pad.exists(), "une instruction ne doit pas écrire dans le bloc-notes");
    let (raw, text): (String, String) = rusqlite::Connection::open(&db).unwrap()
        .query_row("SELECT raw, text FROM history", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    let _ = std::fs::remove_file(&db);
    assert_eq!((raw.as_str(), text.as_str()), ("[instruction] mets en majuscules", "LE CHAT DORT"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_invented_phrase_on_noise_is_not_an_instruction() {
    // Whisper "hears" Thank you. on noise: the online model must not be called, the selection stays intact.
    let (stt, _) = FakeStt::new("Thank you.");
    let (typed, notes, calls) = (Typed::default(), Notes::default(), Default::default());
    let mut svc = DictationService::new(FakeMic::one(white_noise(0.01)), stt, typed.clone(), None, Box::new(notes.clone()))
        .with_instruct(Box::new(Selected(Some("le chat dort".into()))), Box::new(FakeRewriter(std::sync::Arc::clone(&calls))));
    press(&mut svc, vec![InstructStart, InstructStop]);
    assert!(typed.texts().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(notes.all().contains("Nothing heard"), "{}", notes.all());
}

#[test]
fn a_muted_microphone_never_reaches_the_online_model() {
    let (stt, stt_calls) = FakeStt::new("mets en majuscules");
    let (typed, notes, calls) = (Typed::default(), Notes::default(), Default::default());
    let mut svc = DictationService::new(FakeMic::one(muted()), stt, typed.clone(), None, Box::new(notes.clone()))
        .with_instruct(Box::new(Selected(Some("le chat dort".into()))), Box::new(FakeRewriter(std::sync::Arc::clone(&calls))));
    press(&mut svc, vec![InstructStart, InstructStop]);
    assert!(typed.texts().is_empty());
    assert_eq!((calls.load(Ordering::SeqCst), stt_calls.load(Ordering::SeqCst)), (0, 0));
    assert!(notes.all().contains("No sound"), "{}", notes.all());
}

#[test]
fn dictation_and_instruction_can_follow_each_other_in_one_session() {
    let mic = FakeMic { fail: false, takes: [voice(), voice()].into() };
    let (stt, _) = FakeStt::confirmed("bonjour tout le monde");
    let (typed, notes, calls) = (Typed::default(), Notes::default(), Default::default());
    let mut svc = DictationService::new(mic, stt, typed.clone(), None, Box::new(notes.clone()))
        .with_instruct(Box::new(Selected(Some("a b".into()))), Box::new(FakeRewriter(std::sync::Arc::clone(&calls))));
    press(&mut svc, vec![RecordStart, RecordStop, InstructStart, InstructStop]);
    assert_eq!(typed.texts(), vec!["bonjour tout le monde".to_string(), "A B".to_string()], "{}", notes.all());
}
