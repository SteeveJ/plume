use anyhow::Result;
use arboard::Clipboard;
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key};
use std::sync::Mutex;
use std::time::Duration;
use crate::config::APP_NAME;
use crate::domain::ports::injector::TextInjector;

pub struct ClipboardInjector {
    kb: Mutex<evdev::uinput::VirtualDevice>,
}

impl ClipboardInjector {
    pub fn new() -> Result<Self> {
        let kb = make_keyboard()
            .map_err(|e| anyhow::anyhow!("uinput not accessible (check /dev/uinput permissions): {e}"))?;
        // Give the compositor time to register the device once at startup.
        std::thread::sleep(Duration::from_millis(200));
        Ok(Self { kb: Mutex::new(kb) })
    }
}

fn make_keyboard() -> Result<evdev::uinput::VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::KEY_LEFTCTRL);
    keys.insert(Key::KEY_V);
    Ok(VirtualDeviceBuilder::new()?
        .name(APP_NAME)
        .with_keys(&keys)?
        .build()?)
}

impl TextInjector for ClipboardInjector {
    fn inject(&self, text: &str) -> Result<()> {
        let mut cb = Clipboard::new()?;
        cb.set_text(text.to_owned())?;

        // Brief pause so the compositor registers the new clipboard content.
        std::thread::sleep(Duration::from_millis(30));

        self.kb.lock().unwrap().emit(&[
            InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 1),
            InputEvent::new(EventType::KEY, Key::KEY_V.code(), 1),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
            InputEvent::new(EventType::KEY, Key::KEY_V.code(), 0),
            InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 0),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ])?;

        Ok(())
    }
}
