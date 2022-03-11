use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum CtsMessage {
    Text(String),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum StcMessage {
    /// Tells clients that the wolves have woken up.
    WolvesAwake,
}
