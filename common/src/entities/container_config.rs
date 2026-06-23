use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Eq)]
pub struct ContainerConfigV1 {
    pub environment: Option<HashMap<String, String>>,
}
