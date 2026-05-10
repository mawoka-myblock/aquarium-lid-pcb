use picoserve::{
    response::{IntoResponse, StatusCode},
    routing::get,
};

use crate::tasks::http::AppState;

pub fn frontend_router()
-> picoserve::Router<impl picoserve::routing::PathRouter<AppState>, AppState> {
    picoserve::Router::new()
        .route("/", get(get_index))
        .route("/app.js", get(get_js))
        .route("/app.css", get(get_css))
}

async fn get_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("Content-Encoding", "gzip"), ("Content-Type", "text/html")],
        include_bytes!("frontend/dist/index.html.gz").as_slice(),
    )
}
async fn get_js() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            ("Content-Encoding", "gzip"),
            ("Content-Type", "application/javascript"),
        ],
        include_bytes!("frontend/dist/app.js.gz").as_slice(),
    )
}

async fn get_css() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("Content-Encoding", "gzip"), ("Content-Type", "text/css")],
        include_bytes!("frontend/dist/app.css.gz").as_slice(),
    )
}
