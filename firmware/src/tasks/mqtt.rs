use alloc::borrow::ToOwned;
use defmt::{info, unwrap};

use crate::{
    AirData, COMMANDS, DATACHANNEL, StaticMqttClient, StaticMqttState, WaterTemperatureResponse,
};
use embedded_mqttc::QoS;

#[embassy_executor::task]
pub async fn run_mqtt_loop(mqtt_state: &'static StaticMqttState) {
    unwrap!(mqtt_state.run().await);
}

const BASE_TOPIC: &'static str = "aquarium";

#[embassy_executor::task]
pub async fn listen_datachannel(client: StaticMqttClient) {
    let mut sub = DATACHANNEL.subscriber().unwrap();
    let mut buf = [0u8; 512];
    loop {
        let msg = sub.next_message_pure().await;
        let (topic, payload, retain): (&str, &[u8], bool) = match msg {
            crate::DataMessage::AirData(d) => {
                if let Some(data) = d {
                    let d_len = unwrap!(serde_json_core::to_slice::<AirData>(&data, &mut buf));
                    (
                        &(BASE_TOPIC.to_owned() + "/sensor/air/state"),
                        &buf[..d_len],
                        false,
                    )
                } else {
                    continue;
                }
            }
            crate::DataMessage::WaterTemperature(d) => {
                let d_len = unwrap!(serde_json_core::to_slice::<WaterTemperatureResponse>(
                    &WaterTemperatureResponse { temp: Some(d) },
                    &mut buf
                ));
                (
                    &(BASE_TOPIC.to_owned() + "/sensor/water/state"),
                    &buf[..d_len],
                    false,
                )
            }
        };

        info!("publishing: {}", topic);
        match client
            .publish(topic, payload, QoS::AtMostOnce, retain)
            .await
        {
            Ok(_) => (),
            Err(e) => defmt::error!("{:?}", e),
        };
    }
}

#[embassy_executor::task]
pub async fn listen_commandchannel(client: StaticMqttClient) {
    let mut sub = COMMANDS.subscriber().unwrap();
    let mut buf = [0u8; 512];
    loop {
        let msg = sub.next_message_pure().await;
        let (topic, payload, retain): (&str, &[u8], bool) = match msg {
            crate::Command::FanOn => (
                &(BASE_TOPIC.to_owned() + "/sensor/fan/state"),
                "ON".as_bytes(),
                true,
            ),
            crate::Command::FanOff => (
                &(BASE_TOPIC.to_owned() + "/sensor/fan/state"),
                "OFF".as_bytes(),
                true,
            ),
            crate::Command::BuzzerOn => (
                &(BASE_TOPIC.to_owned() + "/sensor/buzzer/state"),
                "ON".as_bytes(),
                true,
            ),
            crate::Command::BuzzerOff => (
                &(BASE_TOPIC.to_owned() + "/sensor/buzzer/state"),
                "OFF".as_bytes(),
                true,
            ),
            crate::Command::Reconfigure(d) => {
                let d_len = unwrap!(serde_json_core::to_slice::<crate::Config>(&d, &mut buf));
                (
                    &(BASE_TOPIC.to_owned() + "/sensor/config/state"),
                    &buf[..d_len],
                    true,
                )
            }
            _ => continue,
        };

        info!("publishing: {}", topic);
        match client
            .publish(topic, payload, QoS::AtLeastOnce, retain)
            .await
        {
            Ok(_) => (),
            Err(e) => defmt::error!("{:?}", e),
        };
    }
}
