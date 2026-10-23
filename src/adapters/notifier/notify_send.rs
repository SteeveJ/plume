use crate::config::APP_NAME;
use crate::domain::ports::notifier::Notifier;

pub struct NotifySend;

impl Notifier for NotifySend {
    fn notify(&self, msg: &str) {
        let _ = notify_rust::Notification::new()
            .summary(APP_NAME)
            .body(msg)
            .id(42099)   // replace-id: reuses the same notification slot
            .timeout(notify_rust::Timeout::Milliseconds(3000))
            .show();
    }
}
