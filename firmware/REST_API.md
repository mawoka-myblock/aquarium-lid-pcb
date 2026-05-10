# Aquarium-Lid REST API

## Config

### Config (/api/config)
**Allowed methods: `GET`, `POST`**


Example:
```json
{
    "fan_on_threshold": 25.3,
    "fan_off_threshold": 25.0,
    "max_safe_temp": 26.0,
    "min_safe_temp": 20.0,
    "alarm_hysteresis": 0.3,
    "led_brightness": 123
}
```


Schema:
```json
{
		"fan_on_threshold": "f32",
		"fan_off_threshold": "f32",
		"max_safe_temp": "f32",
		"min_safe_temp": "f32",
		"alarm_hysteresis": "f32",
		"led_brightness": "u8 (0-255)"
}
```

## Water Temp (/api/data/water)
**Allowed methods: `GET`**


Example:
```json
{
    "temp": 21.5
}
```


Schema:
```json
{
    "temp": "Option<f32>"
}
```


## Fan State (/api/fan)
**Allowed methods: `GET`, `POST`**


Example:
```json
{
    "on": false
}
```


Schema:
```json
{
    "on": "bool"
}
```

## Buzzer State (/api/buzzer)
**Allowed methods: `GET`, `POST`**


Example:
```json
{
    "on": false
}
```


Schema:
```json
{
    "on": "bool"
}
```

## MQTT creds (/api/mqtt)
**Allowed methods: `POST`**


Example:
```json
{
    "host": "192.168.1.128",
    "port": 1883,
    "client_id": "aqlid",
    "username": "MQTT_USERNAME",
    "password": "MQTT_PASSWORD"
}
```


Schema:
```json
{
    "host": "String<64>",
    "port": "Option<u16>",
    "client_id": "String<32>",
    "username": "String<32>",
    "password": "String<64>"
}
```
