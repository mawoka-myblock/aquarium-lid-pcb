use embassy_net::Runner;
use esp_radio::wifi::Interface;

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await
}
