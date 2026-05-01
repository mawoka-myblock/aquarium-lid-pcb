use alloc::borrow::ToOwned;
use defmt::unwrap;

use crate::{AirData, DATACHANNEL, StaticMqttClient, StaticMqttState, WaterTemperatureResponse};
use embedded_mqttc::QoS;

#[embassy_executor::task]
async fn run_mqtt_loop(mqtt_state: &'static StaticMqttState) {
    unwrap!(mqtt_state.run().await);
}

const BASE_TOPIC: &'static str = "aquarium";

#[embassy_executor::task]
async fn listen_datachannel(client: StaticMqttClient) {
    let mut sub = DATACHANNEL.subscriber().unwrap();
    let mut buf = [0u8; 512];
    loop {
        let msg = sub.next_message_pure().await;
        let (topic, payload, retain): (&str, &[u8], bool) = match msg {
            crate::DataMessage::AirData(d) => {
                if let Some(data) = d {
                    unwrap!(serde_json_core::to_slice::<AirData>(&data, &mut buf));
                    (&(BASE_TOPIC.to_owned() + "/sensor/air"), &buf, false)
                } else {
                    continue;
                }
            }
            crate::DataMessage::WaterTemperature(d) => {
                unwrap!(serde_json_core::to_slice::<WaterTemperatureResponse>(
                    &WaterTemperatureResponse { temp: Some(d) },
                    &mut buf
                ));
                (&(BASE_TOPIC.to_owned() + "/sensor/water"), &buf, false)
            }
        };

        match client
            .publish(topic, payload, QoS::AtMostOnce, retain)
            .await
        {
            Ok(_) => (),
            Err(e) => defmt::error!("{:?}", e),
        };
    }
}
