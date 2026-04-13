use esp_hal_smartled::{SmartLedsAdapter, buffer_size};
use smart_leds::{SmartLedsWrite, colors};

use crate::COMMANDS;

#[embassy_executor::task]
pub async fn led_task(leds: &'static mut SmartLedsAdapter<'static, { buffer_size(3) }>) {
    leds.write(core::iter::repeat(colors::WHITE).take(3))
        .unwrap();
    let mut sub = COMMANDS.subscriber().unwrap();
    loop {
        let msg = sub.next_message_pure().await;
        let cmd = match msg {
            crate::Command::SetLeds(d) => d,
            _ => continue,
        };

        match cmd {
            crate::LedCommand::AllColor(d) => leds.write(core::iter::repeat(d).take(3)).unwrap(),
        }
    }
}
