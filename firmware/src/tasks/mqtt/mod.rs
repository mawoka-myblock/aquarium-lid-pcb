use defmt::unwrap;
mod discovery;

pub use discovery::publish_discovery;
use embassy_futures::select::{Either3, select3};
use embassy_time::Timer;
use heapless::Vec;
use rust_mqtt::{
    Bytes,
    client::{
        event::Event,
        options::{PublicationOptions, SubscriptionOptions, TopicReference},
    },
    types::{MqttString, QoS, TopicFilter, TopicName, VarByteInt},
};

use crate::{
    AirData, COMMANDS, DATACHANNEL, MQTT_RECV_CHANNEL, MQTT_SEND_CHANNEL, MqttClientType,
    WaterTemperatureResponse,
};

#[embassy_executor::task]
pub async fn handle_mqtt(client: &'static mut MqttClientType) {
    let mut mqtt_send = MQTT_SEND_CHANNEL.subscriber().unwrap();
    let mqtt_recv = MQTT_RECV_CHANNEL.publisher().unwrap();

    let mut sub_options = SubscriptionOptions::new()
        .retain_handling(rust_mqtt::client::options::RetainHandling::SendIfNotSubscribedBefore)
        .retain_as_published()
        .at_least_once();
    if client.server_config().subscription_identifiers_supported {
        sub_options.subscription_identifier = Some(VarByteInt::from(42u16))
    }
    unwrap!(
        client
            .subscribe(
                TopicFilter::new(MqttString::from_str("aquarium/control/#").unwrap()).unwrap(),
                sub_options,
            )
            .await
    );

    loop {
        match select3(
            mqtt_send.next_message_pure(),
            client.poll_header(),
            Timer::after_secs(4),
        )
        .await
        {
            Either3::First(msg) => {
                unwrap!(
                    client
                        .publish(&msg.0, Bytes::Borrowed(msg.1.as_slice()))
                        .await
                );
                unsafe {
                    client.buffer_mut().reset();
                }
            }
            Either3::Second(event) => {
                let e = unwrap!(event);
                let body = unwrap!(client.poll_body(e).await);
                if let Event::Publish(b) = body {
                    let topic_name = heapless::String::try_from(b.topic.as_ref().as_str()).unwrap();
                    let data = heapless::Vec::from_slice(b.message.as_bytes()).unwrap();
                    mqtt_recv.publish((topic_name, data)).await;
                };
                unsafe {
                    client.buffer_mut().reset();
                }
            }
            Either3::Third(_) => unwrap!(client.ping().await),
        };
    }
}

const BASE_TOPIC: &str = "aquarium";
const AIR_STATE_TOPIC: &str = "aquarium/sensor/air/state";
const WATER_STATE_TOPIC: &str = "aquarium/sensor/water/state";
const FAN_CONTROL_TOPIC: &str = "aquarium/control/fan/state";
const BUZZER_CONTROL_TOPIC: &str = "aquarium/control/buzzer/state";
const CONFIG_SET_TOPIC: &str = "aquarium/sensor/config/state";

#[embassy_executor::task]
pub async fn listen_datachannel() {
    let mut sub = DATACHANNEL.subscriber().unwrap();
    let publisher = MQTT_SEND_CHANNEL.publisher().unwrap();
    let mut buf = [0u8; 512];
    loop {
        let msg = sub.next_message_pure().await;
        let (topic, payload, retain): (&str, &[u8], bool) = match msg {
            crate::DataMessage::AirData(d) => {
                if let Some(data) = d {
                    let d_len = unwrap!(serde_json_core::to_slice::<AirData>(&data, &mut buf));
                    (AIR_STATE_TOPIC, &buf[..d_len], false)
                } else {
                    continue;
                }
            }
            crate::DataMessage::WaterTemperature(d) => {
                let d_len = unwrap!(serde_json_core::to_slice::<WaterTemperatureResponse>(
                    &WaterTemperatureResponse { temp: Some(d) },
                    &mut buf
                ));
                (WATER_STATE_TOPIC, &buf[..d_len], false)
            }
        };

        let mut pub_optns = PublicationOptions::new(TopicReference::Name(
            TopicName::new(MqttString::from_str_unchecked(topic)).unwrap(),
        ))
        .qos(QoS::AtMostOnce);
        if retain {
            pub_optns = pub_optns.retain();
        }
        let d_vec = Vec::from_slice(payload).unwrap();
        publisher.publish((pub_optns, d_vec)).await;
    }
}

#[embassy_executor::task]
pub async fn listen_commandchannel() {
    let mut sub = COMMANDS.subscriber().unwrap();
    let publisher = MQTT_SEND_CHANNEL.publisher().unwrap();
    let mut buf = [0u8; 512];
    loop {
        let msg = sub.next_message_pure().await;
        let (topic, payload, retain): (&str, &[u8], bool) = match msg {
            crate::Command::FanOn => (FAN_CONTROL_TOPIC, "ON".as_bytes(), true),
            crate::Command::FanOff => (FAN_CONTROL_TOPIC, "OFF".as_bytes(), true),
            crate::Command::BuzzerOn => (BUZZER_CONTROL_TOPIC, "ON".as_bytes(), true),
            crate::Command::BuzzerOff => (BUZZER_CONTROL_TOPIC, "OFF".as_bytes(), true),
            crate::Command::Reconfigure(d) => {
                let d_len = unwrap!(serde_json_core::to_slice::<crate::Config>(&d, &mut buf));
                (CONFIG_SET_TOPIC, &buf[..d_len], true)
            }
            _ => continue,
        };

        let mut pub_optns = PublicationOptions::new(TopicReference::Name(
            TopicName::new(MqttString::from_str_unchecked(topic)).unwrap(),
        ))
        .qos(QoS::AtMostOnce);
        if retain {
            pub_optns = pub_optns.retain();
        }
        let d_vec = Vec::from_slice(payload).unwrap();
        publisher.publish((pub_optns, d_vec)).await
    }
}

#[embassy_executor::task]
pub async fn listen_mqtt() {
    let cmd_pub = COMMANDS.immediate_publisher();
    let mut mqtt_sub = MQTT_RECV_CHANNEL.subscriber().unwrap();
    loop {
        let msg = mqtt_sub.next_message_pure().await;

        let topic_name: &str = msg.0.as_str();
        let pl = str::from_utf8(msg.1.as_slice()).unwrap();
        let parts: heapless::Vec<&str, 8> = topic_name.split('/').collect();
        match parts.as_slice() {
            [BASE_TOPIC, "control", device, "set"] => match *device {
                "fan" => match pl {
                    "ON" => cmd_pub.publish_immediate(crate::Command::FanOn),
                    "OFF" => cmd_pub.publish_immediate(crate::Command::FanOff),
                    _ => (),
                },
                "buzzer" => match pl {
                    "ON" => cmd_pub.publish_immediate(crate::Command::BuzzerOn),
                    "OFF" => cmd_pub.publish_immediate(crate::Command::BuzzerOff),
                    _ => (),
                },
                _ => (),
            },
            [BASE_TOPIC, "control", "config", action, "set"] => match *action {
                "fan_above" => {
                    let mut cfg = crate::Config::get_from_signal();
                    cfg.fan_on_threshold = pl.parse::<f32>().unwrap();
                    cmd_pub.publish_immediate(crate::Command::Reconfigure(cfg));
                }
                "fan_below" => {
                    let mut cfg = crate::Config::get_from_signal();
                    cfg.fan_off_threshold = pl.parse::<f32>().unwrap();
                    cmd_pub.publish_immediate(crate::Command::Reconfigure(cfg));
                }
                "max_safe_temp" => {
                    let mut cfg = crate::Config::get_from_signal();
                    cfg.max_safe_temp = pl.parse::<f32>().unwrap();
                    cmd_pub.publish_immediate(crate::Command::Reconfigure(cfg));
                }
                "min_safe_temp" => {
                    let mut cfg = crate::Config::get_from_signal();
                    cfg.min_safe_temp = pl.parse::<f32>().unwrap();
                    cmd_pub.publish_immediate(crate::Command::Reconfigure(cfg));
                }
                "led_brightness" => {
                    let mut cfg = crate::Config::get_from_signal();
                    cfg.led_brightness = pl.parse::<u8>().unwrap();
                    cmd_pub.publish_immediate(crate::Command::Reconfigure(cfg));
                }
                _ => (),
            },
            _ => (),
        }
    }
}
