pub trait ChildInvoker {
    fn invoke_child_async(&self, payload: &[u8]) -> Result<(), String>;
}
