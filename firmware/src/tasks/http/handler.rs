use picoserve::{AppBuilder, routing::PathRouter};

use crate::tasks::http::{api, frontend};

impl AppBuilder for super::AppProps {
    type PathRouter = impl PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        let Self { state } = self;

        picoserve::Router::new()
            .nest("", frontend::frontend_router())
            .nest("/api", api::api_router())
            .with_state(state)
    }
}
