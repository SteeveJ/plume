#[cfg_attr(test, mockall::automock)]
pub trait Notifier: Send + Sync {
    fn notify(&self, msg: &str);
}
