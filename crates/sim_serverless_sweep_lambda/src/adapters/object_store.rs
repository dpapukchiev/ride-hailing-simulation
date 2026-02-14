pub trait OutcomeStore {
    fn write_object(&self, key: &str, body: &[u8]) -> Result<(), String>;
    fn delete_object(&self, key: &str) -> Result<(), String>;
}
