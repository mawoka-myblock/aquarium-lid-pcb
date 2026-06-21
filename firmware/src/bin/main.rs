#![no_main]
#![no_std]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

// use aht20::AHT20;
use bt_hci::controller::ExternalController;
use defmt::{expect, info, unwrap};
use embassy_executor::Spawner;
use embassy_net::tcp::client::{TcpClient, TcpClientState, TcpConnection};
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use embedded_nal_async::TcpConnect;
use esp_ds18b20::Ds18b20;
use esp_hal::gpio::{AnyPin, Input, InputConfig};
use esp_hal::i2c::master::{self as I2C};
use esp_hal::ledc;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::rmt::Rmt;
use esp_hal::rng::Rng;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{Async, ledc::channel::Channel};
use esp_hal::{
    clock::CpuClock,
    ledc::timer::{self, TimerIFace},
};
use esp_hal_smartled::{Ws2812SmartLeds, buffer_size};
use esp_onewire::OneWireBus;
use esp_radio::ble::controller::BleConnector;
use esp_radio::wifi::sta::StationConfig;
use firmware::tasks::fan::{control_fan, fan_task};
use firmware::tasks::http::AppProps;
use firmware::tasks::led::led_task;
use firmware::tasks::measure::water_temp_task;
use firmware::tasks::network::net_task;
use firmware::tasks::safety;
use firmware::tasks::{button, config};
use firmware::{COMMANDS, NvsMutex, mk_static};
use firmware::{MqttClientType, storage::nvs::Nvs};
use firmware::{bt::improv_ble, tasks::mqtt};
use firmware::{
    storage::config::{MqttData, WifiCreds},
    tasks::alarm,
};
use picoserve::AppBuilder;
use rust_mqtt::buffer::BumpBuffer;
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

    let (wifi_controller, interfaces) = esp_radio::wifi::new(peripherals.WIFI, Default::default())
        .expect("Failed to initialize Wi-Fi controller");
    let wifi_controller: &'static mut esp_radio::wifi::WifiController =
        firmware::mk_static!(esp_radio::wifi::WifiController, wifi_controller);
    let transport = BleConnector::new(peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 20>::new(transport);

    let nvs: NvsMutex = firmware::mk_static!(Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Nvs>, Mutex::new(Nvs::new(firmware::NVS_OFFSET, firmware::NVS_SIZE, peripherals.FLASH).unwrap()));

    let cfg: &'static mut firmware::Config =
        firmware::CONFIG.init(firmware::Config::from_nvs(nvs).await);

    // LEDC INIT
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);

    let lstimer0: &'static mut ledc::timer::Timer<'static, LowSpeed> = firmware::mk_static!(
        ledc::timer::Timer<'static, LowSpeed>,
        ledc.timer::<LowSpeed>(timer::Number::Timer0)
    );
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty5Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();

    let fan1: &'static mut Channel<'static, LowSpeed> = firmware::mk_static!(
        Channel<'static, LowSpeed>,
        ledc.channel::<LowSpeed>(ledc::channel::Number::Channel0, peripherals.GPIO9)
    );
    fan1.configure(ledc::channel::config::Config {
        timer: lstimer0,
        duty_pct: 0,
        drive_mode: esp_hal::gpio::DriveMode::PushPull,
    })
    .unwrap();

    let fan2: &'static mut Channel<'static, LowSpeed> = firmware::mk_static!(
        Channel<'static, LowSpeed>,
        ledc.channel::<LowSpeed>(ledc::channel::Number::Channel1, peripherals.GPIO10)
    );
    fan2.configure(ledc::channel::config::Config {
        timer: lstimer0,
        duty_pct: 0,
        drive_mode: esp_hal::gpio::DriveMode::PushPull,
    })
    .unwrap();

    let buzzer_timer: &'static mut ledc::timer::Timer<'static, LowSpeed> = firmware::mk_static!(
        ledc::timer::Timer<'static, LowSpeed>,
        ledc.timer::<LowSpeed>(timer::Number::Timer1)
    );
    unwrap!(buzzer_timer.configure(timer::config::Config {
        duty: timer::config::Duty::Duty10Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_khz(2),
    }));

    let buzzer: &'static mut Channel<'static, LowSpeed> = firmware::mk_static!(
        Channel<'static, LowSpeed>,
        ledc.channel::<LowSpeed>(esp_hal::ledc::channel::Number::Channel3, peripherals.GPIO7)
    );
    buzzer
        .configure(esp_hal::ledc::channel::config::Config {
            timer: buzzer_timer,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();

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
    let addr = loop {
        match ow_bus.find_first_device() {
            Ok(a) => break a,
            Err(e) => {
                defmt::error!("No DS18B20 found, retrying: {:#?}", defmt::Debug2Format(&e));
                Timer::after_millis(500).await;
            }
        }
    };
    let sen: &'static mut Ds18b20 =
        firmware::mk_static!(Ds18b20, Ds18b20::new(addr, ow_bus).unwrap());

    // Button init
    let btn: &'static mut Input<'static> = firmware::mk_static!(
        Input<'static>,
        Input::new(
            peripherals.GPIO6,
            InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        )
    );

    spawner.spawn(fan_task(fan1, fan2).unwrap());
    spawner.spawn(water_temp_task(sen).unwrap());
    spawner.spawn(control_fan(cfg).unwrap());
    spawner.spawn(led_task(led, cfg).unwrap());
    spawner.spawn(config::config_task(cfg, nvs).unwrap());
    spawner.spawn(config::config_mqtt_task(nvs).unwrap());
    spawner.spawn(alarm::buzzer_task(buzzer).unwrap());
    spawner.spawn(alarm::control_alarm(cfg).unwrap());
    spawner.spawn(button::button_task(btn).unwrap());
    spawner.spawn(safety::safety_monitor().unwrap());
    // spawner.spawn(air_data_task(aht)).unwrap();

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
                embassy_net::StackResources<16>,
                embassy_net::StackResources::<16>::new()
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
        unwrap!(wifi_controller.connect_async().await);
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
            let u8_buf = firmware::mk_static!([u8; 1536], [0; 1536]);
            let buffer = firmware::mk_static!(BumpBuffer<'static>, BumpBuffer::new(u8_buf));

            static TCP_CLIENT_STATE: StaticCell<TcpClientState<1, 1024, 1024>> = StaticCell::new();
            let tcp_client_state = TCP_CLIENT_STATE.init(TcpClientState::new());
            static TCP_CLIENT: StaticCell<TcpClient<'static, 1, 1024, 1024>> = StaticCell::new();
            let tcp_client = TCP_CLIENT.init(TcpClient::new(stack, tcp_client_state));

            let dns_client = embassy_net::dns::DnsSocket::new(stack);

            let conn = firmware::mk_static!(
                TcpConnection<'static, 1, 1024, 1024>,
                tcp_client
                    .connect(expect!(
                        mqtt_cfg.get_socket_addr(dns_client).await,
                        "Couldn't get IP. Either DNS failed or IP is invalid for MQTT!"
                    ))
                    .await
                    .unwrap()
            );

            let client = firmware::mk_static!(MqttClientType, MqttClientType::new(buffer));
            unwrap!(
                client
                    .connect(conn, &mqtt_cfg.to_mqtt_client_config(), None)
                    .await
            );

            spawner.spawn(mqtt::handle_mqtt(client).unwrap());
            Timer::after_millis(300).await;
            mqtt::publish_discovery().await;
            spawner.spawn(mqtt::listen_commandchannel().unwrap());
            spawner.spawn(mqtt::listen_datachannel().unwrap());
            spawner.spawn(mqtt::listen_mqtt().unwrap());
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
