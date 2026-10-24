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
}

/// Traduit une valeur evdev (1 = appui, 0 = relâche, 2 = répétition) en évènement d'enregistrement.
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

impl HotkeyListener for EvdevListener {
    /// Ne rend jamais la main : les claviers sont ré-énumérés toutes les 3 s, donc un clavier débranché,
    /// rebranché ou présent seulement après la veille retrouve sa touche sans redémarrer Plume.
    fn listen(&self, tx: Sender<HotkeyEvent>) -> Result<()> {
        let (key, mode) = (self.key, self.mode);
        let toggle = Arc::new(AtomicBool::new(false));
        let active: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
        let mut warned = false;

        loop {
            for (path, mut dev) in evdev::enumerate() {
                if !dev.supported_keys().is_some_and(|k| k.contains(key)) { continue; }
                if !active.lock().unwrap().insert(path.clone()) { continue; }
                warned = false;
                let (tx, toggle, active) = (tx.clone(), Arc::clone(&toggle), Arc::clone(&active));
                thread::spawn(move || {
                    listen_device(&mut dev, key, mode, &toggle, &tx);
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

fn listen_device(dev: &mut Device, key: Key, mode: HotkeyMode, toggle: &AtomicBool, tx: &Sender<HotkeyEvent>) {
    while let Ok(events) = dev.fetch_events() {
        for ev in events {
            if let InputEventKind::Key(k) = ev.kind() {
                if k != key { continue; }
                if let Some(m) = to_event(mode, ev.value(), toggle) { let _ = tx.send(m); }
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
            None => '.',
        }).collect()
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
