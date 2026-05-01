use aht20::AHT20;
use defmt::Debug2Format;
use embassy_time::{Delay, Timer};
use esp_ds18b20::Ds18b20;
use esp_hal::{Async, i2c::master::I2c};

use crate::{AirData, DATACHANNEL, WATER_TEMP_SIGNAL};

#[embassy_executor::task]
pub async fn water_temp_task(ds: &'static mut Ds18b20) {
    let publisher = DATACHANNEL.publisher().unwrap();
    let watch_publisher = WATER_TEMP_SIGNAL.sender();

    loop {
        if let Err(e) = ds.start_temp_measurement() {
            defmt::error!("start_temp_measurement failed: {:?}", Debug2Format(&e));
            Timer::after_millis(200).await;
            continue;
        }

        Timer::after_millis(750).await;

        let mut value = None;

        for attempt in 0..3 {
            match ds.read_sensor_data() {
                Ok(d) => {
                    value = Some(d.temperature);
                    break;
                }
                Err(e) => {
                    defmt::debug!(
                        "read_sensor_data failed (attempt {}): {:?}",
                        attempt + 1,
                        Debug2Format(&e)
                    );
                    Timer::after_millis(50).await;
                }
            }
        }

        match value {
            Some(t) => {
                publisher
                    .publish(crate::DataMessage::WaterTemperature(t))
                    .await;
                watch_publisher.send(t);
            }
            None => {
                defmt::error!("read_sensor_data failed after retries")
            }
        };

        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::task]
pub async fn air_data_task(aht20: &'static mut AHT20<&'static mut I2c<'static, Async>, Delay>) {
    let publisher = DATACHANNEL.publisher().unwrap();
    loop {
        let d = match aht20.measure().await {
            Ok(d) => Some(AirData {
                humidity: d.humidity,
                temperature: d.temperature,
            }),
            Err(e) => {
                defmt::error!("{:?}", Debug2Format(&e));
                None
            }
        };
        publisher.publish(crate::DataMessage::AirData(d)).await;
        Timer::after_millis(5000).await;
    }
}
