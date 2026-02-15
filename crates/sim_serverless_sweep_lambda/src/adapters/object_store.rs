pub trait OutcomeStore {
    fn write_object(&self, key: &str, body: &[u8]) -> Result<(), String>;
}
