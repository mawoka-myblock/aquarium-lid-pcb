#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use aht20::AHT20;
use bt_hci::controller::ExternalController;
use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_ds18b20::Ds18b20;
use esp_hal::gpio::{AnyPin, Output, OutputConfig};
use esp_hal::i2c::master::{self as I2C};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::{Ledc, LowSpeed, channel};
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    clock::CpuClock,
    ledc::timer::{self, TimerIFace},
};
use esp_hal_smartled::{SmartLedsAdapter, buffer_size, smart_led_buffer};
use esp_onewire::OneWireBus;
use esp_radio::ble::controller::BleConnector;
use firmware::storage::nvs::Nvs;
use smart_leds::{SmartLedsWrite, colors};
use trouble_host::prelude::*;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.2.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(&radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // TODO: Spawn some tasks
    let _ = spawner;

    let nvs: &'static Mutex<NoopRawMutex, Nvs> = firmware::mk_static!(Mutex<NoopRawMutex, Nvs>, Mutex::new(Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap()));

    // LEDC INIT
    let mut ledc = Ledc::new(peripherals.LEDC);

    let mut lstimer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty5Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();

    // let mut fan1 = ledc.channel::<LowSpeed>(channel::Number::Channel0, peripherals.GPIO9);
    // fan1.configure(channel::config::Config {
    //     timer: &lstimer0,
    //     duty_pct: 100,
    //     drive_mode: esp_hal::gpio::DriveMode::PushPull,
    // })
    // .unwrap();
    // fan1.set_duty(100).unwrap();

    // let mut fan2 = ledc.channel::<LowSpeed>(channel::Number::Channel1, peripherals.GPIO10);
    // fan2.configure(channel::config::Config {
    //     timer: &lstimer0,
    //     duty_pct: 100,
    //     drive_mode: esp_hal::gpio::DriveMode::PushPull,
    // })
    // .unwrap();
    // fan2.set_duty(100).unwrap();
    let mut fan1 = Output::new(
        peripherals.GPIO9,
        esp_hal::gpio::Level::Low,
        OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
    );
    fan1.set_high();
    let mut fan2 = Output::new(
        peripherals.GPIO10,
        esp_hal::gpio::Level::Low,
        OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
    );
    fan2.set_low();

    let mut buzzer = ledc.channel::<LowSpeed>(channel::Number::Channel2, peripherals.GPIO7);
    buzzer
        .configure(channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();

    // i2c init
    let i2c = I2C::I2c::new(
        peripherals.I2C0,
        I2C::Config::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO20)
    .with_scl(peripherals.GPIO21)
    .into_async();

    // let aht = AHT20::new(i2c, 0x34, embassy_time::Delay).await.unwrap();

    // RGB LED init
    let rmt_buffer: &'static mut [esp_hal::rmt::PulseCode; buffer_size(3)] = firmware::mk_static!(
        [esp_hal::rmt::PulseCode; buffer_size(3)],
        [esp_hal::rmt::PulseCode::default(); buffer_size(3)]
    );
    let mut led = {
        let freq = Rate::from_mhz(80);
        let rmt = Rmt::new(peripherals.RMT, freq).unwrap();
        SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO4, rmt_buffer)
    };
    led.write(core::iter::repeat(colors::RED).take(3)).unwrap();

    // DS18B20 Init
    let ow_pin = AnyPin::from(peripherals.GPIO8);
    let mut ow_bus = OneWireBus::new(ow_pin);
    let addr = ow_bus.find_first_device().unwrap();
    let mut sen = Ds18b20::new(addr, ow_bus).unwrap();

    loop {
        sen.start_temp_measurement().unwrap();
        let wait_time_ms = esp_ds18b20::Resolution::Bits12.measurement_time_ms();
        let wait_time = Duration::from_millis(wait_time_ms as u64);
        Timer::after(wait_time).await;
        let data = sen.read_sensor_data().unwrap();
        info!("Data: {}", data.temperature);
    }
}
