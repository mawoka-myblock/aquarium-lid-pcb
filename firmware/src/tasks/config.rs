use embassy_time::Timer;
use esp_hal::system::software_reset;

use crate::{COMMANDS, CONFIG_SIGNAL, NvsMutex};

#[embassy_executor::task]
pub async fn config_task(cfg: &'static crate::Config, nvs: NvsMutex) {
    let cfg_sender = CONFIG_SIGNAL.sender();
    cfg_sender.send(cfg.clone());
    let mut sub = COMMANDS.subscriber().unwrap();

    loop {
        let msg = sub.next_message_pure().await;
        let config = match msg {
            crate::Command::Reconfigure(d) => d,
            _ => continue,
        };
        config.to_nvs(nvs).await;
        cfg_sender.send(config);
    }
}

#[embassy_executor::task]
pub async fn config_mqtt_task(nvs: NvsMutex) {
    let mut sub = COMMANDS.subscriber().unwrap();
    loop {
        let msg = sub.next_message_pure().await;
        let config = match msg {
            crate::Command::SetMqttConfig(d) => d,
            _ => continue,
        };
        config.to_nvs(nvs).await;
        Timer::after_millis(300).await;
        software_reset()
    }
}
