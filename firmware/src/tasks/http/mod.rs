pub mod api;
mod handler;

use picoserve::AppRouter;

pub const WEB_TASK_POOL_SIZE: usize = 2;

static CONFIG: picoserve::Config = picoserve::Config::const_default().keep_connection_alive();

pub struct AppState {}
pub struct AppProps {
    state: AppState,
}

impl AppProps {
    pub fn new() -> Self {
        Self { state: AppState {} }
    }
}

impl Default for AppProps {
    fn default() -> Self {
        Self::new()
    }
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    task_id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<AppProps>,
) -> ! {
    let port = 8080;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::Server::new(app, &CONFIG, &mut http_buffer)
        .listen_and_serve(task_id, stack, port, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}
