use defmt::{error, info};
use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use smart_leds::colors;

use crate::{COMMANDS, Command, DATACHANNEL, DataMessage, LedCommand};

/// How long we wait for a fresh water-temperature reading before we assume the
/// sensor is broken and force the fan on.
const SENSOR_TIMEOUT_MS: u64 = 20_000;

#[embassy_executor::task]
pub async fn safety_monitor() {
    let mut data_sub = match DATACHANNEL.subscriber() {
        Ok(s) => s,
        Err(_) => {
            error!("Safety monitor: failed to subscribe to data channel");
            return;
        }
    };
    let cmd_pub = match COMMANDS.publisher() {
        Ok(p) => p,
        Err(_) => {
            error!("Safety monitor: failed to get command publisher");
            return;
        }
    };

    let mut failsafe_active = false;

    loop {
        let timeout = Timer::after_millis(SENSOR_TIMEOUT_MS);
        match select(data_sub.next_message_pure(), timeout).await {
            Either::First(DataMessage::WaterTemperature(_)) if failsafe_active => {
                failsafe_active = false;
                info!("Water-temperature reading restored");
                let _ = cmd_pub
                    .publish(Command::SetLeds(LedCommand::Pulse(colors::GREEN)))
                    .await;
                let _ = cmd_pub.publish(Command::BuzzerOff).await;
            }
            Either::First(_) => {}
            Either::Second(_) => {
                if !failsafe_active {
                    failsafe_active = true;
                    error!(
                        "SENSOR TIMEOUT: no water temperature for {} ms — forcing failsafe",
                        SENSOR_TIMEOUT_MS
                    );
                    let _ = cmd_pub.publish(Command::FanOn).await;
                    let _ = cmd_pub.publish(Command::BuzzerOn).await;
                    let _ = cmd_pub
                        .publish(Command::SetLeds(LedCommand::Blink(colors::RED)))
                        .await;
                }
            }
        }
    }
}
