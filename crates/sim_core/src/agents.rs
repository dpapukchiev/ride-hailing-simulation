#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Requesting,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rider {
    pub state: RiderState,
}

impl Rider {
    pub fn new(state: RiderState) -> Self {
        Self { state }
    }
}
