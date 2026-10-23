use anyhow::Result;
use evdev::{Device, InputEventKind, Key};
use std::sync::{mpsc::Sender, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use crate::config::HotkeyMode;
use crate::domain::model::HotkeyEvent;
use crate::domain::ports::hotkey::HotkeyListener;

pub struct RdevListener {
    pub key: Key,
    pub mode: HotkeyMode,
}

impl HotkeyListener for RdevListener {
    fn listen(&self, tx: Sender<HotkeyEvent>) -> Result<()> {
        let key = self.key;
        let mode = self.mode;

        let keyboards: Vec<Device> = evdev::enumerate()
            .filter_map(|(_, d)| {
                if d.supported_keys().map_or(false, |k| k.contains(key)) { Some(d) } else { None }
            })
            .collect();

        anyhow::ensure!(!keyboards.is_empty(), "no keyboard with {key:?} found in /dev/input/");

        let toggle_state = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = keyboards
            .into_iter()
            .map(|mut dev| {
                let tx = tx.clone();
                let toggle = Arc::clone(&toggle_state);
                thread::spawn(move || loop {
                    let Ok(events) = dev.fetch_events() else { break };
                    for ev in events {
                        if let InputEventKind::Key(k) = ev.kind() {
                            if k != key { continue; }
                            let msg = match mode {
                                HotkeyMode::Hold => match ev.value() {
                                    1 => Some(HotkeyEvent::RecordStart),
                                    0 => Some(HotkeyEvent::RecordStop),
                                    _ => None,
                                },
                                HotkeyMode::Toggle => {
                                    if ev.value() == 1 {
                                        let was = toggle.fetch_xor(true, Ordering::SeqCst);
                                        Some(if was { HotkeyEvent::RecordStop } else { HotkeyEvent::RecordStart })
                                    } else {
                                        None
                                    }
                                }
                            };
                            if let Some(m) = msg { let _ = tx.send(m); }
                        }
                    }
                })
            })
            .collect();

        for h in handles { let _ = h.join(); }
        Ok(())
    }
}
