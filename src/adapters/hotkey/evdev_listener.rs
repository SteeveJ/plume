use anyhow::Result;
use evdev::{Device, InputEventKind, Key};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use crate::config::HotkeyMode;
use crate::domain::model::HotkeyEvent;
use crate::domain::ports::hotkey::HotkeyListener;

const RESCAN_EVERY: Duration = Duration::from_secs(3);

pub struct EvdevListener {
    pub key: Key,
    pub mode: HotkeyMode,
    /// Second key (instruction mode), always hold-to-talk: press = start, release = end.
    pub instruct_key: Option<Key>,
}

/// Maps an evdev value (1 = press, 0 = release, 2 = key repeat) to a recording event.
fn to_event(mode: HotkeyMode, value: i32, toggle: &AtomicBool) -> Option<HotkeyEvent> {
    match (mode, value) {
        (HotkeyMode::Hold, 1) => Some(HotkeyEvent::RecordStart),
        (HotkeyMode::Hold, 0) => Some(HotkeyEvent::RecordStop),
        (HotkeyMode::Toggle, 1) => Some(if toggle.fetch_xor(true, Ordering::SeqCst) {
            HotkeyEvent::RecordStop
        } else {
            HotkeyEvent::RecordStart
        }),
        _ => None,
    }
}

fn instruct_event(value: i32) -> Option<HotkeyEvent> {
    match value {
        1 => Some(HotkeyEvent::InstructStart),
        0 => Some(HotkeyEvent::InstructStop),
        _ => None,
    }
}

impl HotkeyListener for EvdevListener {
    /// Never returns: keyboards are re-enumerated every 3 s, so an unplugged,
    /// replugged or post-suspend keyboard gets its key back without restarting Plume.
    fn listen(&self, tx: Sender<HotkeyEvent>) -> Result<()> {
        let (key, mode, instruct) = (self.key, self.mode, self.instruct_key);
        let toggle = Arc::new(AtomicBool::new(false));
        let active: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
        let mut warned = false;

        loop {
            for (path, mut dev) in evdev::enumerate() {
                if !dev.supported_keys().is_some_and(|k| k.contains(key) || instruct.is_some_and(|i| k.contains(i))) { continue; }
                if !active.lock().unwrap().insert(path.clone()) { continue; }
                warned = false;
                let (tx, toggle, active) = (tx.clone(), Arc::clone(&toggle), Arc::clone(&active));
                thread::spawn(move || {
                    listen_device(&mut dev, key, instruct, mode, &toggle, &tx);
                    tracing::warn!("keyboard {} disconnected — watching for it to come back", path.display());
                    active.lock().unwrap().remove(&path);
                });
            }
            if !warned && active.lock().unwrap().is_empty() {
                tracing::warn!("no keyboard with {key:?} found in /dev/input/ — will keep looking");
                warned = true;
            }
            thread::sleep(RESCAN_EVERY);
        }
    }
}

fn listen_device(dev: &mut Device, key: Key, instruct: Option<Key>, mode: HotkeyMode, toggle: &AtomicBool, tx: &Sender<HotkeyEvent>) {
    while let Ok(events) = dev.fetch_events() {
        for ev in events {
            if let InputEventKind::Key(k) = ev.kind() {
                let msg = if k == key { to_event(mode, ev.value(), toggle) } else if Some(k) == instruct { instruct_event(ev.value()) } else { None };
                if let Some(m) = msg { let _ = tx.send(m); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(mode: HotkeyMode, values: &[i32]) -> String {
        let toggle = AtomicBool::new(false);
        values.iter().map(|&v| match to_event(mode, v, &toggle) {
            Some(HotkeyEvent::RecordStart) => 'S',
            Some(HotkeyEvent::RecordStop) => 'E',
            _ => '.',
        }).collect()
    }

    #[test]
    fn instruct_key_is_always_hold_and_ignores_key_repeat() {
        let kinds: String = [1, 2, 2, 0, 5].iter().map(|&v| match instruct_event(v) {
            Some(HotkeyEvent::InstructStart) => 'S',
            Some(HotkeyEvent::InstructStop) => 'E',
            _ => '.',
        }).collect();
        assert_eq!(kinds, "S..E.");
    }

    #[test]
    fn hold_mode_follows_press_and_release_and_ignores_key_repeat() {
        assert_eq!(kinds(HotkeyMode::Hold, &[1, 2, 2, 0]), "S..E");
    }

    #[test]
    fn toggle_mode_alternates_on_each_press_only() {
        assert_eq!(kinds(HotkeyMode::Toggle, &[1, 0, 1, 2, 0, 1]), "S.E..S");
    }
}
