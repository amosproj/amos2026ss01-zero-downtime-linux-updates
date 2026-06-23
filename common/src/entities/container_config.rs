use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq)]
pub struct ContainerConfigV1 {
    pub environment: Option<HashMap<String, String>>,
}

impl Default for ContainerConfigV1 {
    fn default() -> Self {
        Self { environment: None }
    }
}
