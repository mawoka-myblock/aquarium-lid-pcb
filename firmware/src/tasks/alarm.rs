use crate::{AlarmThreshold, BUZZER_ON_SIGNAL, COMMANDS, Command, DATACHANNEL, LedCommand};
use defmt::{error, info};

use embassy_futures::join::join;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use esp_hal::ledc::{
    LowSpeed,
    channel::{Channel, ChannelIFace},
};
use smart_leds::colors;

#[embassy_executor::task]
pub async fn buzzer_task(buzzer: &'static mut Channel<'static, LowSpeed>) {
    let mut sub = COMMANDS.subscriber().unwrap();
    let buzzer_signal_pub = BUZZER_ON_SIGNAL.sender();
    loop {
        let msg = sub.next_message_pure().await;
        match msg {
            Command::BuzzerOn => {
                info!("Buzzer on");
                if let Err(e) = buzzer.set_duty(30) {
                    error!("buzzer.set_duty(30) failed: {:#?}", defmt::Debug2Format(&e));
                }
                buzzer_signal_pub.send(true);
            }
            Command::BuzzerOff => {
                info!("Buzzer off");
                if let Err(e) = buzzer.set_duty(0) {
                    error!("buzzer.set_duty(0) failed: {:#?}", defmt::Debug2Format(&e));
                }
                buzzer_signal_pub.send(false);
            }
            _ => {}
        }
    }
}
static FAN_THRESHOLD_SIGNAL: Signal<CriticalSectionRawMutex, AlarmThreshold> = Signal::new();

#[embassy_executor::task]
pub async fn control_alarm(cfg: &'static crate::Config) {
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
            let current_on = if prev_on {
                // Currently ON:
                // stay ON until we're comfortably back inside the safe range
                temp >= (thresholds.max_safe_temp - thresholds.alarm_hysteresis)
                    || temp <= (thresholds.min_safe_temp + thresholds.alarm_hysteresis)
            } else {
                // Currently OFF:
                // only turn ON when clearly outside the safe range
                temp >= (thresholds.max_safe_temp + thresholds.alarm_hysteresis)
                    || temp <= (thresholds.min_safe_temp - thresholds.alarm_hysteresis)
            };

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
    fn from_cfg(config: &crate::Config) -> Self {
        AlarmThreshold {
            max_safe_temp: config.max_safe_temp,
            min_safe_temp: config.min_safe_temp,
            alarm_hysteresis: config.alarm_hysteresis,
        }
    }
}
