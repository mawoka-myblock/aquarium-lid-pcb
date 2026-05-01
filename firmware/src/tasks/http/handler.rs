use picoserve::{
    AppBuilder,
    routing::{PathRouter, get_service},
};

use crate::tasks::http::api;

impl AppBuilder for super::AppProps {
    type PathRouter = impl PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        let Self { state } = self;

        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html("hallo welt")),
            )
            .nest("/api", api::api_router())
            .with_state(state)
    }
}
