use crate::{COMMANDS, Command, DATACHANNEL, FAN_STATE_ON_SIGNAL, FanThreshold, LedCommand};
use defmt::{error, info};
use embassy_futures::join::join;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use esp_hal::ledc::{
    LowSpeed,
    channel::{Channel, ChannelIFace},
};
use smart_leds::colors;

#[embassy_executor::task]
pub async fn fan_task(
    fan1: &'static mut Channel<'static, LowSpeed>,
    fan2: &'static mut Channel<'static, LowSpeed>,
) {
    let mut sub = COMMANDS.subscriber().unwrap();
    let signal_pub = FAN_STATE_ON_SIGNAL.sender();
    loop {
        let msg = sub.next_message_pure().await;
        match msg {
            Command::FanOn => {
                if let Err(e) = fan1.set_duty(100) {
                    error!("fan1.set_duty failed: {:#?}", defmt::Debug2Format(&e));
                }
                if let Err(e) = fan2.set_duty(100) {
                    error!("fan2.set_duty failed: {:#?}", defmt::Debug2Format(&e));
                }
                info!("Fan turned on");
                signal_pub.send(true);
            }
            Command::FanOff => {
                if let Err(e) = fan1.set_duty(0) {
                    error!("fan1.set_duty(0) failed: {:#?}", defmt::Debug2Format(&e));
                }
                if let Err(e) = fan2.set_duty(0) {
                    error!("fan2.set_duty(0) failed: {:#?}", defmt::Debug2Format(&e));
                }
                info!("Fan turned off");
                signal_pub.send(false);
            }
            _ => {}
        }
    }
}

static FAN_THRESHOLD_SIGNAL: Signal<CriticalSectionRawMutex, FanThreshold> = Signal::new();

#[embassy_executor::task]
pub async fn control_fan(cfg: &'static crate::Config) {
    let data_loop = async {
        let mut sub_data = DATACHANNEL.subscriber().unwrap();
        let cmd_pub = COMMANDS.publisher().unwrap();
        let mut thresholds = FanThreshold::from_cfg(cfg);
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
                        .publish(Command::SetLeds(LedCommand::AllColor(colors::ORANGE)))
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
                Command::Reconfigure(d) => d,
                _ => continue,
            };
            FAN_THRESHOLD_SIGNAL.signal(FanThreshold::from_cfg(&d));
        }
    };
    join(data_loop, cmd_loop).await;
}

impl FanThreshold {
    fn from_cfg(config: &crate::Config) -> Self {
        FanThreshold {
            fan_on: config.fan_on_threshold,
            fan_off: config.fan_off_threshold,
        }
    }
}
