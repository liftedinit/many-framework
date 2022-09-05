use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Migration {
    pub issue: Option<String>,
    pub block_height: u64,
}
