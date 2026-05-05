pub mod util;

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
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogResponseEntry<'a> {
        pub name: &'a str,
        pub version: &'a str,
        pub url: &'a str,
        pub signature: Base64<'a>,
    }
}
