#[cfg_attr(test, mockall::automock)]
pub trait ContextReader: Send + Sync {
    /// Returns text currently in the focused text field before the cursor (~2000 chars max).
    fn get_context(&self) -> Option<String>;
}
