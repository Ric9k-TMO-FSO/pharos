// SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
// SPDX-FileCopyrightText: 2026 Ric9k
//
// SPDX-License-Identifier: BSD-3-Clause

//! Pharos: steady, dim light on TOHs with AW2023 controller, to find the phone in the dark.
//!
//! Derived from the blinker example of Jolla's symbiosis (BSD-3-Clause):
//! https://github.com/sailfishos/symbiosis/tree/main/examples/blinker
//! Uses the AW2023 direct (constant) mode instead of the
//! pattern mode, so the light stays steady even at very low brightness.

mod controller;

use crate::controller::LedController;
use argh::FromArgs;
use futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use symbiosis::toh::*;
use tokio::time::sleep;
use zbus::fdo::Error as DBusError;

type BoxError = Box<dyn Error>;

/// Front LED pattern, defined in /etc/mce/90-pharos.ini. MCE shows it only
/// while the display is off (ScreenOn = 0), so it only needs activating once.
const FRONT_PATTERN: &str = "PatternPharos";

#[derive(FromArgs)]
#[argh(description = "Steady dim TOH light")]
struct Arguments {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Off(Off),
    On(On),
    Watch(Watch),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "off", description = "Turn off the LEDs")]
struct Off {}

#[derive(FromArgs)]
#[argh(subcommand, name = "on", description = "Turn on a steady light")]
struct On {
    /// brightness, 1-255
    #[argh(positional)]
    brightness: u8,

    /// LED current step, 0-15 (default 15, as in Jolla's breathing effect)
    #[argh(option, default = "15")]
    current: u8,

    /// which of the three TOH LEDs to light, 0-2 (default 1)
    #[argh(option, default = "1")]
    led: u8,
}

#[derive(FromArgs)]
#[argh(
    subcommand,
    name = "watch",
    description = "Light up while the display is off, turn off when it is on"
)]
struct Watch {
    /// brightness, 1-255 (default 3)
    #[argh(option, default = "3")]
    brightness: u8,

    /// LED current step, 0-15 (default 15)
    #[argh(option, default = "15")]
    current: u8,

    /// which of the three TOH LEDs to light, 0-2 (default 1)
    #[argh(option, default = "1")]
    led: u8,
}

#[derive(Copy, Clone, Debug)]
enum Action {
    On { brightness: u8, current: u8, led: u8 },
    Off,
}

/// Finds the TOH, checks it has an AW2023 controller and applies the action.
async fn apply(action: Action) -> Result<(), BoxError> {
    let mut toh = Toh::new().await?;
    let needs_power_on = !(&toh).is_powered().await?;
    match (&toh).detect().await {
        Ok(Some(info)) => {
            match info.extra.get("led-controller") {
                Some(ExtraValue::Text(controller)) if controller == "aw2023" => {}
                Some(ExtraValue::Text(controller)) => {
                    return Err(format!("Unknown controller: {controller}").into())
                }
                _ => return Err("This is not the TOH we are looking for".into()),
            }
            // Leave power on while the light is on.
            let leave_powered = matches!(action, Action::On { .. });
            // The library makes sure we use the borrowing interface correctly here.
            if let Err(error) = toh
                .access_i2c_dev_with_power(leave_powered, move |dev| {
                    Box::pin(async move {
                        let mut ctrl = LedController::new(dev);
                        if needs_power_on {
                            sleep(Duration::from_millis(500)).await;
                        }
                        match action {
                            Action::On {
                                brightness,
                                current,
                                led,
                            } => ctrl.start_constant(brightness, current, led).await,
                            Action::Off => ctrl.turn_led_off(),
                        }
                    })
                })
                .await?
            {
                Err(format!("Failed to set LED: {error}").into())
            } else {
                Ok(())
            }
        }
        Ok(None) => Err("TOH is not present or is of unsupported type".into()),
        Err(FetchingInfoError::DBus(DBusError::ServiceUnknown(..))) => {
            Err("TOH daemon is not running".into())
        }
        Err(FetchingInfoError::DBus(error)) => Err(format!("D-Bus error: {:?}", error).into()),
        Err(FetchingInfoError::Conversion(error)) => {
            Err(format!("Conversion error: {error}").into())
        }
    }
}

/// Activates or deactivates the front LED pattern in MCE.
async fn front_led(active: bool) -> Result<(), BoxError> {
    let connection = zbus::Connection::system().await?;
    let method = if active {
        "req_led_pattern_activate"
    } else {
        "req_led_pattern_deactivate"
    };
    connection
        .call_method(
            Some("com.nokia.mce"),
            "/com/nokia/mce/request",
            Some("com.nokia.mce.request"),
            method,
            &(FRONT_PATTERN,),
        )
        .await?;
    Ok(())
}

/// Front LED errors are reported but never stop the TOH light.
async fn front_led_or_warn(active: bool) {
    if let Err(error) = front_led(active).await {
        eprintln!("Front LED: {error}");
    }
}

/// Follows MCE display state: light on when the display is off, off otherwise.
async fn watch(brightness: u8, current: u8, led: u8) -> Result<(), BoxError> {
    let connection = zbus::Connection::system().await?;

    // Subscribe before reading the current state, so no change is missed.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("com.nokia.mce.signal")?
        .member("display_status_ind")?
        .build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;

    let reply = connection
        .call_method(
            Some("com.nokia.mce"),
            "/com/nokia/mce/request",
            Some("com.nokia.mce.request"),
            "get_display_status",
            &(),
        )
        .await?;
    let mut status: String = reply.body().deserialize()?;

    front_led_or_warn(true).await;

    // None until the first successful update.
    let mut lit: Option<bool> = None;
    loop {
        let off = status == "off";
        if lit != Some(off) {
            let action = if off {
                Action::On {
                    brightness,
                    current,
                    led,
                }
            } else {
                Action::Off
            };
            match apply(action).await {
                Ok(()) => {
                    println!("Display {status}: light {}", if off { "on" } else { "off" });
                    lit = Some(off);
                }
                Err(error) => eprintln!("Display {status}: {error}"),
            }
        }

        match signals.next().await {
            Some(Ok(message)) => {
                if let Ok(new_status) = message.body().deserialize::<String>() {
                    status = new_status;
                }
            }
            Some(Err(error)) => eprintln!("Signal error: {error}"),
            None => return Err("Display signal stream closed".into()),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BoxError> {
    let args: Arguments = argh::from_env();

    match args.command {
        Command::On(on) => {
            println!(
                "Turning on steady light: brightness={} current={} led={}",
                on.brightness, on.current, on.led
            );
            front_led_or_warn(true).await;
            apply(Action::On {
                brightness: on.brightness,
                current: on.current,
                led: on.led,
            })
            .await
        }
        Command::Off(_) => {
            println!("Turning LED off");
            front_led_or_warn(false).await;
            apply(Action::Off).await
        }
        Command::Watch(watch_args) => {
            watch(watch_args.brightness, watch_args.current, watch_args.led).await
        }
    }
}
