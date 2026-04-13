use embassy_sync::pubsub::Subscriber;
use embassy_time::{Duration, Timer};
use defmt::info;
use crate::{Message, Settings};
use esp_ds18b20::Ds18b20;
use esp_onewire::OneWireBus;
use esp_hal::gpio::AnyPin;

#[embassy_executor::task]
pub async fn fan_task(mut sub: Subscriber<Message, 4>) {
    // Hard‑coded temperature thresholds
    let fan_on_threshold = 28.0_f32;   // °C
    let fan_off_threshold = 26.0_f32; // °C

    let mut fans_on = false;

    // Example 1‑wire setup – replace GPIO number with the real pin
    let ow_pin = AnyPin::new(8); // placeholder; use correct peripheral access
    let ow_bus = OneWireBus::new(ow_pin);
    let addr = ow_bus.find_first_device().await.unwrap();
    let mut sensor = Ds18b20::new(addr, ow_bus).await.unwrap();

    loop {
        sensor.start_temp_measurement().await.unwrap();
        Timer::after(Duration::from_millis(750)).await;
        let data = sensor.read_sensor_data().await.unwrap();
        let temp = data.temperature;
        info!("Water temperature: {}°C", temp);

        if temp > fan_on_threshold && !fans_on {
            sub.publisher().publish(Message::FanOn);
            fans_on = true;
        } else if temp < fan_off_threshold && fans_on {
            sub.publisher().publish(Message::FanOff);
            fans_on = false;
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}
