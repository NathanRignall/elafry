use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Message {
    pub channel_id: u32,
    pub data: Vec<u8>,
    pub count: u32,
    pub timestamp: u64,
}
