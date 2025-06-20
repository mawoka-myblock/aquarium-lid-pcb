#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Input;
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::Pull;
use esp_hal::ledc::channel;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::timer::TimerSpeed;
use esp_hal::ledc::Ledc;
use esp_hal::ledc::LowSpeed;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{smart_led_buffer, SmartLedsAdapter};
use esp_println::println;
use log::info;
use smart_leds::SmartLedsWrite;
use smart_leds::RGB;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    // generator version: 0.3.1

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let _init = esp_wifi::init(
        timer1.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();
    println!("Hallo Welt!1");
    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80)).unwrap();
    println!("Hallo Welt!1.1");
    let rmt_buffer = smart_led_buffer!(3);
    println!("Hallo Welt!1.2");
    let mut led = SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO4, rmt_buffer);
    println!("Hallo Welt!1.3");
    // led.write(
    //     core::iter::repeat(RGB {
    //         r: 255u8,
    //         g: 0u8,
    //         b: 0u8,
    //     })
    //     .take(4),
    // )
    // .unwrap();
    println!("Hallo Welt!2");

    let ledc = Ledc::new(peripherals.LEDC);
    let mut ledtimer1 = ledc.timer::<LowSpeed>(esp_hal::ledc::timer::Number::Timer0);
    ledtimer1
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(1),
        })
        .unwrap();
    let mut channel0 =
        ledc.channel::<LowSpeed>(esp_hal::ledc::channel::Number::Channel0, peripherals.GPIO7);
    channel0
        .configure(channel::config::Config {
            timer: &ledtimer1,
            duty_pct: 80,
            pin_config: channel::config::PinConfig::PushPull,
        })
        .unwrap();
    println!("Hallo Welt!3");
    Timer::after(Duration::from_secs(1)).await;
    println!("Hallo Welt!4");
    channel0.set_duty(0).unwrap();

    // TODO: Spawn some tasks
    let _ = spawner;

    let btn = Input::new(peripherals.GPIO6, InputConfig::default().with_pull(Pull::Up));

    loop {
        Timer::after(Duration::from_secs(1)).await;
        println!("Hallo Welt!");
        let btn_status = btn.is_high();
        println!("Btn is high: {:?}", btn_status)
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-beta.0/examples/src/bin
}
