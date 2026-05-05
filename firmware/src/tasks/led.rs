use embassy_time::Timer;
use esp_hal::Async;
use esp_hal_smartled::{Ws2812SmartLeds, buffer_size};
use smart_leds::{RGB8, SmartLedsWriteAsync, brightness, colors};

use crate::COMMANDS;

#[embassy_executor::task]
pub async fn led_task(
    leds: &'static mut Ws2812SmartLeds<'static, { buffer_size::<RGB8>(3) }, Async>,
    cfg: &'static crate::Config,
) {
    let mut sub = COMMANDS.subscriber().unwrap();

    let mut effect = Effect::Solid {
        color: RGB8::new(255, 255, 255),
    };

    let mut brightness_val = cfg.led_brightness;

    let mut frame = [RGB8::new(0, 0, 0); LEDS];

    loop {
        // Handle commands (non-blocking)
        if let Some(msg) = sub.try_next_message_pure() {
            if let crate::Command::SetLeds(cmd) = msg {
                effect = match cmd {
                    crate::LedCommand::AllColor(c) => Effect::Solid { color: c },
                    crate::LedCommand::Pulse(c) => Effect::Pulse { color: c, step: 0 },
                    crate::LedCommand::Blink(c) => Effect::Blink { color: c, on: true },
                    crate::LedCommand::BlinkIdentify => Effect::Blink {
                        color: colors::BLUE_VIOLET,
                        on: true,
                    },
                };
            } else if let crate::Command::Reconfigure(c) = msg {
                brightness_val = c.led_brightness
            }
        }

        // Run effect
        effect.update(&mut frame);

        leds.write(brightness(frame.iter().copied(), brightness_val))
            .await
            .unwrap();

        // Sleep based on effect needs
        Timer::after_millis(effect.interval_ms()).await;
    }
}

const LEDS: usize = 3;

#[derive(Clone, Copy)]
pub enum Effect {
    Solid { color: RGB8 },
    Pulse { color: RGB8, step: usize },
    Blink { color: RGB8, on: bool },
}

impl Effect {
    pub fn update(&mut self, out: &mut [RGB8; LEDS]) {
        match self {
            Effect::Solid { color } => {
                fill(out, *color);
            }

            Effect::Pulse { color, step } => {
                let v = BREATH[*step];
                let b = wave_to_brightness(v);
                let scaled = RGB8 {
                    r: GAMMA8[((color.r as u16 * b as u16) >> 8) as usize],
                    g: GAMMA8[((color.g as u16 * b as u16) >> 8) as usize],
                    b: GAMMA8[((color.b as u16 * b as u16) >> 8) as usize],
                };
                fill(out, scaled);
                *step = (*step + 2) % BREATH.len();
            }

            Effect::Blink { color, on } => {
                if *on {
                    fill(out, *color);
                } else {
                    fill(out, RGB8::new(0, 0, 0));
                }
                *on = !*on;
            }
        }
    }

    /// Controls how often this effect needs updates
    pub fn interval_ms(&self) -> u64 {
        match self {
            Effect::Solid { .. } => 200, // basically idle
            Effect::Pulse { .. } => 30,  // smooth animation
            Effect::Blink { .. } => 500, // slow toggle
        }
    }
}

fn fill(buf: &mut [RGB8; LEDS], color: RGB8) {
    for led in buf.iter_mut() {
        *led = color;
    }
}

fn _brightness_s(c: RGB8, b: u8) -> RGB8 {
    RGB8::new(
        (c.r as u16 * (b as u16 + 1) / 255) as u8,
        (c.g as u16 * (b as u16 + 1) / 255) as u8,
        (c.b as u16 * (b as u16 + 1) / 255) as u8,
    )
}

const GAMMA8: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5,
    5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14,
    14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24, 24, 25, 25, 26, 27,
    27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46,
    47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 72,
    73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104,
    105, 107, 109, 110, 112, 114, 115, 117, 119, 120, 122, 124, 126, 127, 129, 131, 133, 135, 137,
    138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 167, 169, 171, 173, 175,
    177, 180, 182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213, 215, 218, 220,
    223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];

fn _gamma_s(c: RGB8) -> RGB8 {
    RGB8 {
        r: GAMMA8[c.r as usize],
        g: GAMMA8[c.g as usize],
        b: GAMMA8[c.b as usize],
    }
}

sine_macro::sine_wave! {
    pub const BREATH = sine_wave(frequency: 1, rate: 256, type: i16);
}

fn wave_to_brightness(v: i16) -> u8 {
    // map [-32768, 32767] → [0, 255]
    (((v as i32 + 32768) * 255) / 65535) as u8
}
