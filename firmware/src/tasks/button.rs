use async_button::{Button, ButtonConfig, ButtonEvent};
use esp_hal::gpio::Input;

use crate::{COMMANDS, FAN_STATE_ON_SIGNAL};

#[embassy_executor::task]
pub async fn button_task(phy_btn: &'static mut Input<'static>) {
    let command_pub = COMMANDS.publisher().unwrap();
    let mut btn = Button::new(phy_btn, ButtonConfig::default());
    loop {
        match btn.update().await {
            ButtonEvent::LongPress => {
                command_pub.publish(crate::Command::BuzzerOff).await;
            }
            ButtonEvent::ShortPress { count } => match count {
                2 => match FAN_STATE_ON_SIGNAL.try_get() {
                    Some(d) => match d {
                        true => command_pub.publish(crate::Command::FanOff).await,
                        false => command_pub.publish(crate::Command::FanOn).await,
                    },
                    None => command_pub.publish(crate::Command::FanOn).await,
                },
                _ => (),
            },
        }
    }
}
