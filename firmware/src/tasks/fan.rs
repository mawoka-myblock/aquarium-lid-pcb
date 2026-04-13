use crate::{COMMANDS, Command, DATACHANNEL, FanThreshold, LedCommand};
use defmt::info;

use embassy_futures::join::join;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Instant;
use esp_hal::gpio::Output;
use smart_leds::colors;

#[embassy_executor::task]
pub async fn fan_task(fan1: &'static mut Output<'static>, fan2: &'static mut Output<'static>) {
    let mut sub = COMMANDS.subscriber().unwrap();
    loop {
        let msg = sub.next_message_pure().await;
        match msg {
            Command::FanOn => {
                fan1.set_high();
                fan2.set_high();
                info!("Fan turned on");
            }
            Command::FanOff => {
                fan1.set_low();
                fan2.set_low();
                info!("Fan turned off");
            }
            _ => {}
        }
    }
}

static FAN_THRESHOLD_SIGNAL: Signal<CriticalSectionRawMutex, FanThreshold> = Signal::new();

#[embassy_executor::task]
pub async fn control_fan() {
    let data_loop = async {
        let mut sub_data = DATACHANNEL.subscriber().unwrap();
        let cmd_pub = COMMANDS.publisher().unwrap();
        let mut thresholds = FanThreshold {
            fan_off: 22.0,
            fan_on: 23.0,
        };
        let mut prev_on = false;
        loop {
            if let Some(new_thr) = FAN_THRESHOLD_SIGNAL.try_take() {
                thresholds = new_thr;
            }
            let msg = sub_data.next_message_pure().await;
            let temp = match msg {
                crate::DataMessage::WaterTemperature(d) => d,
                _ => continue,
            };
            info!("Current temp: {}", temp);
            let current_on = if temp >= thresholds.fan_on {
                true
            } else if temp <= thresholds.fan_off {
                false
            } else {
                prev_on
            };

            if current_on != prev_on {
                if current_on {
                    cmd_pub.publish(Command::FanOn).await;
                    cmd_pub
                        .publish(Command::SetLeds(LedCommand::AllColor(colors::RED)))
                        .await;
                } else {
                    cmd_pub.publish(Command::FanOff).await;
                    cmd_pub
                        .publish(Command::SetLeds(LedCommand::AllColor(colors::GREEN)))
                        .await;
                }
                prev_on = current_on;
            }
        }
    };

    let cmd_loop = async {
        let mut sub_data = COMMANDS.subscriber().unwrap();
        loop {
            let msg = sub_data.next_message_pure().await;
            let d = match msg {
                Command::SetFanThreshold(d) => d,
                _ => continue,
            };
            FAN_THRESHOLD_SIGNAL.signal(d);
        }
    };
    join(data_loop, cmd_loop).await;
}
