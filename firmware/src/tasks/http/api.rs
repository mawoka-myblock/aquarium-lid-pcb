use picoserve::{
    extract::Json,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    BUZZER_ON_SIGNAL, COMMANDS, CONFIG_SIGNAL, FAN_STATE_ON_SIGNAL, WATER_TEMP_SIGNAL,
    storage::config::MqttData, tasks::http::AppState,
};

pub fn api_router() -> picoserve::Router<impl picoserve::routing::PathRouter<AppState>, AppState> {
    picoserve::Router::new()
        .route("/config", get(get_config).post(set_config))
        .route("/data/water", get(get_water_temperature))
        .route("/fan", get(get_fan).post(set_fan))
        .route("/buzzer", get(get_buzzer).post(set_buzzer))
        .route("/mqtt", post(set_mqtt))
}

async fn get_config() -> impl IntoResponse {
    let mut recv = CONFIG_SIGNAL.anon_receiver();
    picoserve::response::Json(recv.try_get().unwrap())
}

async fn set_config(Json(cfg): Json<crate::Config>) -> impl IntoResponse {
    COMMANDS
        .immediate_publisher()
        .publish_immediate(crate::Command::Reconfigure(cfg.clone()));
    picoserve::response::Json(cfg)
}

#[derive(Serialize, Debug)]
struct WaterTemperatureResponse {
    temp: Option<f32>,
}

async fn get_water_temperature() -> impl IntoResponse {
    let mut recv = WATER_TEMP_SIGNAL.anon_receiver();
    picoserve::response::Json(WaterTemperatureResponse {
        temp: recv.try_get(),
    })
}

#[derive(Deserialize, Serialize, Debug)]
struct SetFan {
    on: bool,
}

async fn set_fan(Json(fan_data): Json<SetFan>) -> impl IntoResponse {
    let cmd = match fan_data.on {
        true => crate::Command::FanOn,
        false => crate::Command::FanOff,
    };
    COMMANDS.immediate_publisher().publish_immediate(cmd);
    picoserve::response::Json(fan_data)
}

async fn get_fan() -> impl IntoResponse {
    picoserve::response::Json(SetFan {
        on: FAN_STATE_ON_SIGNAL
            .anon_receiver()
            .try_get()
            .unwrap_or_default(),
    })
}

async fn set_mqtt(Json(data): Json<MqttData>) -> impl IntoResponse {
    COMMANDS
        .immediate_publisher()
        .publish_immediate(crate::Command::SetMqttConfig(data));
}

async fn set_buzzer(Json(fan_data): Json<SetFan>) -> impl IntoResponse {
    let cmd = match fan_data.on {
        true => crate::Command::BuzzerOn,
        false => crate::Command::BuzzerOff,
    };
    COMMANDS.immediate_publisher().publish_immediate(cmd);
    picoserve::response::Json(fan_data)
}

async fn get_buzzer() -> impl IntoResponse {
    picoserve::response::Json(SetFan {
        on: BUZZER_ON_SIGNAL
            .anon_receiver()
            .try_get()
            .unwrap_or_default(),
    })
}
