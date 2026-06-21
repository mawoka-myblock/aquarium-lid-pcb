#![no_std]
#![feature(impl_trait_in_assoc_type)]
use embassy_net::tcp::client::TcpConnection;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, pubsub::PubSubChannel, watch::Watch,
};
use heapless::String;
use rust_mqtt::{
    buffer::BumpBuffer,
    client::{Client, options::PublicationOptions},
};
use serde::{Deserialize, Serialize};
use smart_leds::RGB8;
use static_cell::StaticCell;

use crate::storage::{config::MqttData, nvs::Nvs};

extern crate alloc;

pub mod bt;
pub mod storage;
pub mod tasks;

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

pub const NVS_OFFSET: usize = 0x9000;
pub const NVS_SIZE: usize = 0x6000;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    fan_on_threshold: f32,
    fan_off_threshold: f32,
    max_safe_temp: f32,
    min_safe_temp: f32,
    alarm_hysteresis: f32,
    led_brightness: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    FanOn,
    FanOff,
    SetLeds(LedCommand),
    BuzzerOn,
    BuzzerOff,
    Reconfigure(Config),
    SetMqttConfig(MqttData),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LedCommand {
    AllColor(RGB8),
    Pulse(RGB8),
    Blink(RGB8),
    BlinkIdentify,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AirData {
    pub temperature: f32,
    pub humidity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FanThreshold {
    pub fan_on: f32,
    pub fan_off: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlarmThreshold {
    pub min_safe_temp: f32,
    pub max_safe_temp: f32,
    pub alarm_hysteresis: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataMessage {
    WaterTemperature(f32),
    AirData(Option<AirData>),
}

pub type CommandChannel = PubSubChannel<CriticalSectionRawMutex, Command, 4, 8, 7>;

pub static COMMANDS: CommandChannel = CommandChannel::new();

pub type DataChannel = PubSubChannel<CriticalSectionRawMutex, DataMessage, 4, 4, 4>;

pub static MQTT_RECV_CHANNEL: PubSubChannel<
    CriticalSectionRawMutex,
    (String<64>, heapless::Vec<u8, 512>),
    3,
    1,
    1,
> = PubSubChannel::new();

pub static MQTT_SEND_CHANNEL: PubSubChannel<
    CriticalSectionRawMutex,
    (PublicationOptions, heapless::Vec<u8, 512>),
    3,
    1,
    2,
> = PubSubChannel::new();

pub static DATACHANNEL: DataChannel = DataChannel::new();

pub static CONFIG: StaticCell<Config> = StaticCell::new();

pub static CONFIG_SIGNAL: Watch<CriticalSectionRawMutex, Config, 2> = Watch::new();

pub static WATER_TEMP_SIGNAL: Watch<CriticalSectionRawMutex, f32, 1> = Watch::new();

pub static FAN_STATE_ON_SIGNAL: Watch<CriticalSectionRawMutex, bool, 1> = Watch::new();

pub static BUZZER_ON_SIGNAL: Watch<CriticalSectionRawMutex, bool, 1> = Watch::new();

pub type NvsMutex = &'static Mutex<CriticalSectionRawMutex, Nvs>;

#[derive(Serialize, Debug)]
pub struct WaterTemperatureResponse {
    pub temp: Option<f32>,
}

pub type MqttClientType = Client<
    'static,
    &'static mut TcpConnection<'static, 1, 1024, 1024>,
    BumpBuffer<'static>,
    1,
    1,
    1,
    16,
>;

/// Custom panic-halt callback for `esp-backtrace`.
///
/// Instead of dead-locking the CPU in an empty loop after a panic, we reboot.
/// This is a last-resort safety net so the fish never get cooked because the
/// firmware got stuck.
#[unsafe(no_mangle)]
extern "Rust" fn custom_halt() -> ! {
    defmt::error!("PANIC — rebooting instead of freezing");
    esp_hal::system::software_reset();
}
