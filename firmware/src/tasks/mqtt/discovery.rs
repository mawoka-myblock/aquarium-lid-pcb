use heapless::Vec;
use rust_mqtt::{
    client::options::{PublicationOptions, TopicReference},
    types::{MqttString, QoS, TopicName},
};

use crate::MQTT_SEND_CHANNEL;

const WATER_TEMP_CFG: &str = r#"
{
"name":"Water Temperature",
"stat_t":"aquarium/sensor/water/state",
"val_tpl":"{{value_json.temp}}",
"uniq_id":"aq_w_temp",
"unit_of_meas":"°C",
"sug_dsp_prc":1,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const AIR_TEMP_CFG: &str = r#"
{
"name":"Air Temperature",
"stat_t":"aquarium/sensor/air/state",
"val_tpl":"{{value_json.temperature}}",
"uniq_id":"aq_a_temp",
"unit_of_meas":"°C",
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const AIR_HUM_CFG: &str = r#"
{
"name":"Air Humidity",
"stat_t":"aquarium/sensor/air/state",
"val_tpl":"{{value_json.humidity}}",
"uniq_id":"aq_a_hum",
"unit_of_meas":"%",
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const FAN_SWITCH: &str = r#"
{
"name":"Fan Switch",
"stat_t":"aquarium/control/fan/state",
"cmd_t":"aquarium/control/fan/set",
"pl_on":"ON",
"pl_off":"OFF",
"uniq_id":"aq_fan",
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const BUZZER_SWITCH: &str = r#"
{
"name":"Buzzer Switch",
"stat_t":"aquarium/control/buzzer/state",
"cmd_t":"aquarium/control/buzzer/set",
"pl_on":"ON",
"pl_off":"OFF",
"uniq_id":"aq_buzzer",
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const FAN_ON_ABOVE: &str = r#"
{
"name":"Fan On Threshold",
"stat_t":"aquarium/sensor/config/state",
"cmd_t":"aquarium/control/config/fan_above/set",
"val_tpl": "{{value_json.fan_on_threshold}}",
"uniq_id":"aq_fan_above",
"unit_of_meas":"°C",
"mode":"box",
"step":0.1,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const FAN_OFF_BELOW: &str = r#"
{
"name":"Fan Off Threshold",
"stat_t":"aquarium/sensor/config/state",
"cmd_t":"aquarium/control/config/fan_below/set",
"val_tpl": "{{value_json.fan_off_threshold}}",
"uniq_id":"aq_fan_below",
"unit_of_meas":"°C",
"mode":"box",
"step":0.1,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const LED_BRIGHTNESS: &str = r#"
{
"name":"LED Brightness",
"stat_t":"aquarium/sensor/config/state",
"cmd_t":"aquarium/control/config/led_brightness/set",
"val_tpl": "{{value_json.led_brightness}}",
"uniq_id":"aq_led_brightness",
"mode":"box",
"min":0,
"max":255,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const MAX_SAFE_TEMP: &str = r#"
{
"name":"Alarm on above",
"stat_t":"aquarium/sensor/config/state",
"cmd_t":"aquarium/control/config/max_safe_temp/set",
"val_tpl": "{{value_json.max_safe_temp}}",
"uniq_id":"aq_max_safe_temp",
"unit_of_meas":"°C",
"mode":"box",
"step":0.1,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

const MIN_SAFE_TEMP: &str = r#"
{
"name":"Alarm off below",
"stat_t":"aquarium/sensor/config/state",
"cmd_t":"aquarium/control/config/min_safe_temp/set",
"val_tpl": "{{value_json.min_safe_temp}}",
"uniq_id":"aq_min_safe_temp",
"unit_of_meas":"°C",
"mode":"box",
"step":0.1,
"dev":{"ids":"aq", "name":"Aquarium Fan"}
}
"#;

pub async fn publish_discovery() {
    let client = MQTT_SEND_CHANNEL.publisher().unwrap();
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/sensor/aquarium_water/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(WATER_TEMP_CFG.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/sensor/aquarium_air_temp/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(AIR_TEMP_CFG.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/sensor/aquarium_air_hum/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(AIR_HUM_CFG.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/switch/aquarium_fan/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(FAN_SWITCH.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/switch/aquarium_buzzer/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(BUZZER_SWITCH.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/number/aquarium_fan_on_thres/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(FAN_ON_ABOVE.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/number/aquarium_fan_off_thres/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(FAN_OFF_BELOW.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/number/aquarium_led_brightness/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(LED_BRIGHTNESS.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/number/aquarium_max_safe_temp/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(MAX_SAFE_TEMP.as_bytes()).unwrap(),
        ))
        .await;
    client
        .publish((
            PublicationOptions::new(TopicReference::Name(
                TopicName::new(MqttString::from_str_unchecked(
                    "homeassistant/number/aquarium_min_safe_temp/config",
                ))
                .unwrap(),
            ))
            .qos(QoS::AtMostOnce)
            .retain(),
            Vec::from_slice(MIN_SAFE_TEMP.as_bytes()).unwrap(),
        ))
        .await;
}
