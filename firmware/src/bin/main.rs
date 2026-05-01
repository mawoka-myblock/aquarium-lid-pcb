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

// use aht20::AHT20;
use bt_hci::controller::ExternalController;
use defmt::{Debug2Format, info};
use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use embedded_mqttc::ClientConfig;
use esp_ds18b20::Ds18b20;
use esp_hal::Async;
use esp_hal::gpio::{AnyPin, Output, OutputConfig};
use esp_hal::i2c::master::{self as I2C};
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::rmt::Rmt;
use esp_hal::rng::Rng;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    clock::CpuClock,
    ledc::timer::{self, TimerIFace},
};
use esp_hal_smartled::{Ws2812SmartLeds, buffer_size};
use esp_onewire::OneWireBus;
use esp_radio::ble::controller::BleConnector;
use esp_radio::wifi::sta::StationConfig;
use firmware::bt::improv_ble;
use firmware::storage::config::{MqttData, WifiCreds};
use firmware::storage::nvs::Nvs;
use firmware::tasks::config::config_task;
use firmware::tasks::fan::{control_fan, fan_task};
use firmware::tasks::http::AppProps;
use firmware::tasks::led::led_task;
use firmware::tasks::measure::water_temp_task;
use firmware::tasks::network::net_task;
use firmware::{COMMANDS, NvsMutex, mk_static};
use picoserve::AppBuilder;
use static_cell::StaticCell;

use picoserve::AppRouter;
use smart_leds::{RGB8, colors};
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

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

    // let radio_init: &'static Controller<'_> = &*firmware::mk_static!(
    //     Controller,
    //     esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    // );
    let (wifi_controller, interfaces) = esp_radio::wifi::new(peripherals.WIFI, Default::default())
        .expect("Failed to initialize Wi-Fi controller");
    let wifi_controller: &'static mut esp_radio::wifi::WifiController =
        firmware::mk_static!(esp_radio::wifi::WifiController, wifi_controller);
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 20>::new(transport);

    // Set up Pub/Sub channel and spawn tasks
    // Fan task listens on pubsub

    let nvs: NvsMutex = firmware::mk_static!(Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Nvs>, Mutex::new(Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap()));

    let cfg: &'static mut firmware::Config =
        firmware::CONFIG.init(firmware::Config::from_nvs(nvs).await);

    // LEDC INIT
    let ledc = Ledc::new(peripherals.LEDC);

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
    let _buzzer = Output::new(
        peripherals.GPIO7,
        esp_hal::gpio::Level::Low,
        OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
    );

    // i2c init
    let _i2c: &'static mut I2C::I2c<'static, Async> = firmware::mk_static!(
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
    let led: &'static mut Ws2812SmartLeds<'static, { buffer_size::<RGB8>(3) }, Async> = firmware::mk_static!(
        Ws2812SmartLeds<'static, { buffer_size::<RGB8>(3) }, Async>,
        {
            let freq = Rate::from_mhz(80);
            let rmt = Rmt::new(peripherals.RMT, freq).unwrap().into_async();
            Ws2812SmartLeds::new(rmt.channel0, peripherals.GPIO4).unwrap()
        }
    );

    // DS18B20 Init
    let ow_pin = AnyPin::from(peripherals.GPIO8);
    let mut ow_bus = OneWireBus::new(ow_pin);
    let addr = ow_bus.find_first_device().unwrap();
    let sen: &'static mut Ds18b20 =
        firmware::mk_static!(Ds18b20, Ds18b20::new(addr, ow_bus).unwrap());

    spawner.spawn(fan_task(fan1, fan2).unwrap());
    spawner.spawn(water_temp_task(sen).unwrap());
    spawner.spawn(control_fan(cfg).unwrap());
    spawner.spawn(led_task(led, cfg).unwrap());
    spawner.spawn(config_task(cfg, nvs).unwrap());
    // spawner.spawn(air_data_task(aht)).unwrap();
    //
    Timer::after_millis(1000).await;
    COMMANDS
        .publisher()
        .unwrap()
        .publish(firmware::Command::SetLeds(firmware::LedCommand::Pulse(
            colors::AQUA,
        )))
        .await;

    let wifi_creds = WifiCreds::from_nvs(nvs).await;
    let stack;
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    if let Some(creds) = wifi_creds {
        let mut dhcp_cfg = embassy_net::DhcpConfig::default();

        let mut hostname = heapless::String::new();
        hostname.push_str("aquarium.local").unwrap();
        dhcp_cfg.hostname = Some(hostname);
        let config = embassy_net::Config::dhcpv4(dhcp_cfg);
        let (s_stack, runner) = embassy_net::new(
            interfaces.station,
            config,
            firmware::mk_static!(
                embassy_net::StackResources<12>,
                embassy_net::StackResources::<12>::new()
            ),
            seed,
        );
        stack = s_stack;
        let sta_cfg = esp_radio::wifi::Config::Station(
            StationConfig::default()
                .with_ssid(creds.ssid.as_str())
                .with_password(creds.password.as_str().into()),
        );
        wifi_controller.set_config(&sta_cfg).unwrap();
        wifi_controller
            .set_power_saving(esp_radio::wifi::PowerSaveMode::Minimum)
            .unwrap();
        match wifi_controller.connect_async().await {
            Err(e) => {
                defmt::error!("{:?}", Debug2Format(&e));
                panic!();
            }
            _ => (),
        };
        spawner.spawn(net_task(runner).unwrap());
        info!("Waiting for IP...");
        loop {
            if let Some(config) = stack.config_v4() {
                info!("Got IP: {}", config.address);
                break;
            }
            Timer::after_millis(200).await;
        }
        let app = mk_static!(AppRouter<AppProps>, AppProps::new().build_app());
        for task_id in 0..firmware::tasks::http::WEB_TASK_POOL_SIZE {
            spawner.spawn(firmware::tasks::http::web_task(task_id, stack, app).unwrap());
        }
        let mqtt_cfg_optn: &'static Option<MqttData> =
            mk_static!(Option<MqttData>, MqttData::from_nvs(nvs).await);
        if let Some(mqtt_cfg) = mqtt_cfg_optn {
            static TCP_CLIENT_STATE: StaticCell<TcpClientState<1, 1024, 1024>> = StaticCell::new();
            let tcp_client_state = TCP_CLIENT_STATE.init(TcpClientState::new());
            static TCP_CLIENT: StaticCell<TcpClient<'static, 1, 1024, 1024>> = StaticCell::new();
            let tcp_client = TCP_CLIENT.init(TcpClient::new(stack, tcp_client_state));
            let dns_client = DnsSocket::new(stack);
            static MQTT_STATE: StaticCell<firmware::StaticMqttState> = StaticCell::new();
            let mqtt_state = MQTT_STATE.init_with(|| {
                firmware::StaticMqttState::new(
                    mqtt_cfg.to_mqtt_client_config(),
                    None,
                    tcp_client,
                    dns_client,
                )
            });
        }
    } else {
        spawner.spawn(improv_ble(ble_controller, wifi_controller, interfaces, nvs).unwrap());
        Timer::after_millis(300).await;
        COMMANDS
            .publisher()
            .unwrap()
            .publish(firmware::Command::SetLeds(firmware::LedCommand::Pulse(
                colors::BLUE,
            )))
            .await;
        loop {
            Timer::after_millis(100).await
        }
    }

    loop {
        // sen.start_temp_measurement().unwrap();
        // let wait_time_ms = esp_ds18b20::Resolution::Bits12.measurement_time_ms();
        // let wait_time = Duration::from_millis(wait_time_ms as u64);
        // Timer::after(wait_time).await;
        // let data = sen.read_sensor_data().unwrap();
        // info!("Data: {}", data.temperature);
        Timer::after_millis(1000).await
    }
}
