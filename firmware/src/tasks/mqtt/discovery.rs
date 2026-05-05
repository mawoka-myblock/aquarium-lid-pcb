use embedded_mqttc::QoS;

use crate::StaticMqttClient;

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

const AIR_HUM_CFF: &str = r#"
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

pub async fn publish_discovery(client: StaticMqttClient) {
    client
        .publish(
            "homeassistant/sensor/aquarium_water/config",
            WATER_TEMP_CFG.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/sensor/aquarium_air_temp/config",
            AIR_TEMP_CFG.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/sensor/aquarium_air_hum/config",
            AIR_HUM_CFF.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/switch/aquarium_fan/config",
            FAN_SWITCH.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/switch/aquarium_buzzer/config",
            BUZZER_SWITCH.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();

    client
        .publish(
            "homeassistant/number/aquarium_fan_on_thres/config",
            FAN_ON_ABOVE.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/number/aquarium_fan_off_thres/config",
            FAN_OFF_BELOW.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
    client
        .publish(
            "homeassistant/number/aquarium_led_brightness/config",
            LED_BRIGHTNESS.as_bytes(),
            QoS::AtLeastOnce,
            true,
        )
        .await
        .unwrap();
}
