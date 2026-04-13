use aht20::AHT20;
use defmt::Debug2Format;
use embassy_time::{Delay, Timer};
use esp_ds18b20::Ds18b20;
use esp_hal::{Async, i2c::master::I2c};

use crate::{AirData, DATACHANNEL};

#[embassy_executor::task]
pub async fn water_temp_task(ds: &'static mut Ds18b20) {
    let publisher = DATACHANNEL.publisher().unwrap();
    loop {
        ds.start_temp_measurement().unwrap();
        Timer::after_millis(1000).await;
        let res = match ds.read_sensor_data() {
            Ok(d) => d.temperature,
            Err(e) => {
                defmt::error!("{:?}", Debug2Format(&e));
                -255.0
            }
        };
        publisher
            .publish(crate::DataMessage::WaterTemperature(res))
            .await;
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
