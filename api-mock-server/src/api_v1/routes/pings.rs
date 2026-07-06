use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err,
    pagination::{Page, PageParams},
    pagination_err,
};
use axum::{
    Json, Router,
    extract::Query,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes() -> Router {
    Router::new().route("/pings", get(list_pings))
}

/// GET /pings — List device pings.
/// Optional query: `?page=1&page_size=20`
async fn list_pings(Query(page): Query<PageParams>) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_pings(page.to_db_page(), page.page_size).await {
        Ok((data, total)) => {
            Json(Page::new(data, page.page, page.page_size, total)).into_response()
        }
        Err(e) => db_err(e),
    }
}
