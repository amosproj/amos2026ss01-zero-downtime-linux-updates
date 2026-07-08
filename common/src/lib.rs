pub mod device_api;
pub mod device_jwt;
pub mod entities;
pub mod util;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize)]
pub struct Page<T> {
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub total_pages: u64,
    pub data: Vec<T>,
}

impl<T: Serialize + DeserializeOwned> Page<T> {
    pub fn new(data: Vec<T>, page: u64, page_size: u64, total_items: u64) -> Self {
        let total_pages = total_items.div_ceil(page_size.max(1));
        Self {
            page,
            page_size,
            total_items,
            total_pages,
            data,
        }
    }
}
