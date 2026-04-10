#![no_std]

extern crate alloc;

pub mod mm_task;
pub mod storage;

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
