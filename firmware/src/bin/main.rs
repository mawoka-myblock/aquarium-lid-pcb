#![no_main]
#![no_std]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

// --- move tasks to separate modules ---

// Unified Pub/Sub channel for inter-task communication

use aht20::AHT20;
use bt_hci::controller::ExternalController;
use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_ds18b20::Ds18b20;
use esp_hal::Async;
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
use firmware::tasks::fan::{control_fan, fan_task};
use firmware::tasks::led::led_task;
use firmware::tasks::measure::{air_data_task, water_temp_task};

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

    // Set up Pub/Sub channel and spawn tasks
    // Fan task listens on pubsub

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
    let fan1: &'static mut Output<'static> = firmware::mk_static!(
        Output<'static>,
        Output::new(
            peripherals.GPIO9,
            esp_hal::gpio::Level::Low,
            OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
        )
    );
    let fan2: &'static mut Output<'static> = firmware::mk_static!(
        Output<'static>,
        Output::new(
            peripherals.GPIO10,
            esp_hal::gpio::Level::Low,
            OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
        )
    );

    // let mut buzzer = ledc.channel::<LowSpeed>(channel::Number::Channel0, peripherals.GPIO7);
    // buzzer
    //     .configure(channel::config::Config {
    //         timer: &lstimer0,
    //         duty_pct: 80,
    //         drive_mode: esp_hal::gpio::DriveMode::PushPull,
    //     })
    //     .unwrap();
    let buzzer = Output::new(
        peripherals.GPIO7,
        esp_hal::gpio::Level::Low,
        OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
    );

    // i2c init
    let i2c: &'static mut I2C::I2c<'static, Async> = firmware::mk_static!(
        I2C::I2c<'static, Async>,
        I2C::I2c::new(
            peripherals.I2C0,
            I2C::Config::default().with_frequency(Rate::from_khz(100)),
        )
        .unwrap()
        .with_sda(peripherals.GPIO20)
        .with_scl(peripherals.GPIO21)
        .into_async()
    );

    // let aht: &'static mut AHT20<&'static mut I2C::I2c<'static, Async>, embassy_time::Delay> = firmware::mk_static!(
    //     AHT20<&'static mut I2C::I2c<'static, Async>, embassy_time::Delay>,
    //     AHT20::new(i2c, 0x34, embassy_time::Delay).await.unwrap()
    // );

    // RGB LED init
    let rmt_buffer: &'static mut [esp_hal::rmt::PulseCode; buffer_size(3)] = firmware::mk_static!(
        [esp_hal::rmt::PulseCode; buffer_size(3)],
        [esp_hal::rmt::PulseCode::default(); buffer_size(3)]
    );
    let led: &'static mut SmartLedsAdapter<'static, { buffer_size(3) }> =
        firmware::mk_static!(SmartLedsAdapter<'static, { buffer_size(3) }>, {
            let freq = Rate::from_mhz(80);
            let rmt = Rmt::new(peripherals.RMT, freq).unwrap();
            SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO4, rmt_buffer)
        });

    // DS18B20 Init
    let ow_pin = AnyPin::from(peripherals.GPIO8);
    let mut ow_bus = OneWireBus::new(ow_pin);
    let addr = ow_bus.find_first_device().unwrap();
    let sen: &'static mut Ds18b20 =
        firmware::mk_static!(Ds18b20, Ds18b20::new(addr, ow_bus).unwrap());

    spawner.spawn(fan_task(fan1, fan2)).unwrap();
    spawner.spawn(water_temp_task(sen)).unwrap();
    spawner.spawn(control_fan()).unwrap();
    spawner.spawn(led_task(led)).unwrap();
    // spawner.spawn(air_data_task(aht)).unwrap();

    loop {
        // sen.start_temp_measurement().unwrap();
        let wait_time_ms = esp_ds18b20::Resolution::Bits12.measurement_time_ms();
        // let wait_time = Duration::from_millis(wait_time_ms as u64);
        // Timer::after(wait_time).await;
        // let data = sen.read_sensor_data().unwrap();
        // info!("Data: {}", data.temperature);
        Timer::after_millis(1000).await
    }
}
