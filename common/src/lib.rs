pub mod util;

pub mod api {
    use crate::util::Base64;
    use serde::{Deserialize, Serialize};

    // GET /v1/catalog
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogResponse<'a> {
        #[serde(borrow)]
        pub os: CatalogResponseEntry<'a>,
        #[serde(borrow)]
        pub app: CatalogResponseEntry<'a>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogResponseEntry<'a> {
        pub version: &'a str,
        pub url: &'a str,
        pub signature: Base64<'a>,
    }
}
