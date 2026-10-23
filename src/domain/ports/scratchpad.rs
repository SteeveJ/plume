#[cfg_attr(test, mockall::automock)]
pub trait ScratchpadWriter: Send + Sync {
    fn append(&self, text: &str);
}
