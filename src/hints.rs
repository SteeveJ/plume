/// Short explanation and action for the most common startup failures
/// (Plume starts without a terminal: without a notification the user sees nothing).
pub fn startup_hint(error: &str) -> Option<&'static str> {
    let e = error.to_lowercase();
    if e.contains("uinput") {
        Some("Add yourself to the 'input' group, install assets/99-plume.rules, then log out and back in.")
    } else if e.contains("hotkey") {
        Some("Fix the HOTKEY line in ~/.config/plume.conf (example: HOTKEY=KEY_RIGHTCTRL).")
    } else if e.contains("api key") {
        Some("Add OPENAI_API_KEY to ~/.config/plume.conf, or check your connection so the local speech model can download.")
    } else if e.contains("home not set") {
        Some("Start Plume from a normal user session.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_failures_get_an_actionable_hint() {
        assert!(startup_hint("init text injector: uinput not accessible").unwrap().contains("input"));
        assert!(startup_hint("unknown HOTKEY 'KEY_X'").unwrap().contains("HOTKEY"));
        assert!(startup_hint("OpenAI API key not found").unwrap().contains("OPENAI_API_KEY"));
        assert_eq!(startup_hint("something else"), None);
    }
}
