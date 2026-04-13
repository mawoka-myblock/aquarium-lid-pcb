#![no_std]
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    pubsub::PubSubChannel,
};
use smart_leds::RGB8;

extern crate alloc;

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

pub struct Settings {
    fan_on_threshold: f32,
    fan_off_threshold: f32,
    alarm_above: f32,
    alarm_below: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    FanOn,
    FanOff,
    BuzzerPulse,
    Reconfigure,
    SetFanThreshold(FanThreshold),
    SetLeds(LedCommand),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LedCommand {
    AllColor(RGB8),
}

#[derive(Debug, Clone, PartialEq)]
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
pub enum DataMessage {
    WaterTemperature(f32),
    AirData(Option<AirData>),
}

pub type CommandChannel = PubSubChannel<CriticalSectionRawMutex, Command, 4, 4, 4>;

pub static COMMANDS: CommandChannel = CommandChannel::new();

pub type DataChannel = PubSubChannel<CriticalSectionRawMutex, DataMessage, 4, 4, 4>;

pub static DATACHANNEL: DataChannel = DataChannel::new();
