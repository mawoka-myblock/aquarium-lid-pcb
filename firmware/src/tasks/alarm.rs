use crate::{AlarmThreshold, COMMANDS, Command, DATACHANNEL, LedCommand};
use defmt::info;

use embassy_futures::join::join;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use esp_hal::gpio::Output;
use smart_leds::colors;

#[embassy_executor::task]
pub async fn fan_task(_buzzer: &'static mut Output<'static>) {
    let mut sub = COMMANDS.subscriber().unwrap();
    loop {
        let msg = sub.next_message_pure().await;
        match msg {
            Command::BuzzerOn => {
                info!("Buzzer turned on");
            }
            Command::BuzzerOff => {
                info!("Buzzer turned off");
            }
            _ => {}
        }
    }
}

static FAN_THRESHOLD_SIGNAL: Signal<CriticalSectionRawMutex, AlarmThreshold> = Signal::new();

#[embassy_executor::task]
pub async fn control_fan(cfg: &'static crate::Config) {
    let data_loop = async {
        let mut sub_data = DATACHANNEL.subscriber().unwrap();
        let cmd_pub = COMMANDS.publisher().unwrap();
        let mut thresholds = AlarmThreshold::from_cfg(cfg);
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
            let current_on = temp > thresholds.alarm_above || temp < thresholds.alarm_below;

            if current_on != prev_on {
                if current_on {
                    cmd_pub.publish(Command::BuzzerOn).await;
                    cmd_pub
                        .publish(Command::SetLeds(LedCommand::AllColor(colors::RED)))
                        .await;
                } else {
                    cmd_pub.publish(Command::BuzzerOff).await;
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
                Command::Reconfigure(d) => d,
                _ => continue,
            };
            FAN_THRESHOLD_SIGNAL.signal(AlarmThreshold::from_cfg(&d));
        }
    };
    join(data_loop, cmd_loop).await;
}

impl AlarmThreshold {
    fn from_cfg(cfg: &crate::Config) -> Self {
        Self {
            alarm_above: cfg.alarm_above,
            alarm_below: cfg.alarm_below,
        }
    }
}
