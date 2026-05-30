use serde::Serialize;

/// Query params: `?page=1&page_size=20`
/// Index 1 is the first page
#[derive(Debug, Clone, Copy)]
pub struct PageParams {
    pub page: u64,
    pub page_size: u64,
}
/// Default values for page and page_size used in query structs with
/// `#[serde(default = "pagination::default_page")]` and `#[serde(default = "pagination::default_page_size")]`
pub fn default_page() -> u64 {
    1
}

pub fn default_page_size() -> u64 {
    20
}

impl PageParams {
    pub fn new(page: u64, page_size: u64) -> Self {
        Self { page, page_size }
    }
    /// Convert 1-based page index to 0-based offset
    pub fn to_db_page(self) -> u64 {
        self.page - 1
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.page < 1 {
            return Err("page must be >= 1");
        }
        if self.page_size == 0 {
            return Err("page_size must be > 0");
        }
        if self.page_size > 200 {
            return Err("page_size must be <= 200");
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct Page<T: Serialize> {
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub total_pages: u64,
    pub data: Vec<T>,
}

impl<T: Serialize> Page<T> {
    pub fn new(data: Vec<T>, params: PageParams, total_items: u64) -> Self {
        let total_pages = total_items.div_ceil(params.page_size.max(1));
        Self {
            page: params.page,
            page_size: params.page_size,
            total_items,
            total_pages,
            data,
        }
    }
}
