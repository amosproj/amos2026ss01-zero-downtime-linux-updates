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

pub mod api {
    use crate::util::Base64;
    use serde::{Deserialize, Serialize};

    // GET /v1/catalog
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogResponse<'a>(
        #[serde(borrow)] pub std::borrow::Cow<'a, [CatalogResponseEntry<'a>]>,
    );

    impl<'a> CatalogResponse<'a> {
        pub const fn from_slice(slice: &'a [CatalogResponseEntry<'a>]) -> Self {
            Self(std::borrow::Cow::Borrowed(slice))
        }

        pub const fn from_owned(vec: Vec<CatalogResponseEntry<'a>>) -> Self {
            Self(std::borrow::Cow::Owned(vec))
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogResponseEntry<'a> {
        pub name: &'a str,
        pub version: &'a str,
        pub url: &'a str,
        pub signature: Base64<'a>,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        static CATALOG_TEST: &str =
            r#"[{"name":"test","version":"1.0.0","url":"https://hallo.welt/","signature":"BQUF"}]"#;

        #[test]
        fn test_catalog_serialization() {
            let catalog = CatalogResponse::from_owned(vec![CatalogResponseEntry {
                name: "test",
                version: "1.0.0",
                url: "https://hallo.welt/",
                signature: Base64::from_slice(&[5u8; 3]),
            }]);

            let result = serde_json::to_string(&catalog).unwrap();

            if result != CATALOG_TEST {
                panic!("Wrong serialization: {}", result)
            }
        }

        #[test]
        fn test_catalog_parsing() {
            let catalog: CatalogResponse = serde_json::from_str(CATALOG_TEST).unwrap();

            assert!(catalog.0.len() == 1);
            assert!(catalog.0[0].name == "test");
            assert!(catalog.0[0].version == "1.0.0");
            assert!(catalog.0[0].url == "https://hallo.welt/");
            assert!(*catalog.0[0].signature == [5u8; 3]);
        }
    }
}
