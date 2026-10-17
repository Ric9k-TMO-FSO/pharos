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

// The AW2023 register access lives in src/controller.rs.
mod controller;

// Our small driver for the TOH LED chip.
use crate::controller::LedController;
// Command line parsing: derives the parser from the structs below.
use argh::FromArgs;
// Gives `.next()` on the D-Bus signal stream.
use futures::StreamExt;
// Generic error type, so different libraries' errors can be returned alike.
use std::error::Error;
// Time spans, for the short wait after powering the TOH on.
use std::time::Duration;
// Symbiosis client: TOH detection, TOH info and I²C bus borrowing.
use symbiosis::toh::*;
// Asynchronous sleep that does not block the program.
use tokio::time::sleep;
// D-Bus error type, to recognise "symbiosis is not running".
use zbus::fdo::Error as DBusError;

// Shorthand for "any error, boxed".
type BoxError = Box<dyn Error>;

/// Front LED pattern, defined in /etc/mce/90-pharos.ini. MCE shows it only
/// while the display is off (ScreenOn = 0), so it only needs activating once.
const FRONT_PATTERN: &str = "PatternPharos";

// Top-level command line: `pharos <command>`.
#[derive(FromArgs)]
// Text shown by `pharos --help`.
#[argh(description = "Steady dim TOH light")]
struct Arguments {
    // The chosen sub-command (on, off or watch).
    #[argh(subcommand)]
    command: Command,
}

// The three sub-commands.
#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    // `pharos off`
    Off(Off),
    // `pharos on <brightness>`
    On(On),
    // `pharos watch`
    Watch(Watch),
}

// `pharos off`: no options.
#[derive(FromArgs)]
#[argh(subcommand, name = "off", description = "Turn off the LEDs")]
struct Off {}

// `pharos on <brightness> [--current N] [--led N]`: light up now, for tests.
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

// `pharos watch [--brightness N] [--current N] [--led N]`: what the service runs.
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

// What to do with the TOH LED; Copy so it can be moved into the async closure.
#[derive(Copy, Clone, Debug)]
enum Action {
    // Steady light with these settings.
    On { brightness: u8, current: u8, led: u8 },
    // LED chip off.
    Off,
}

/// Finds the TOH, checks it has an AW2023 controller and applies the action.
async fn apply(action: Action) -> Result<(), BoxError> {
    // Connect to symbiosis (Jolla's TOH daemon) over D-Bus.
    let mut toh = Toh::new().await?;
    // Remember whether the TOH was unpowered: the chip then needs time to start.
    let needs_power_on = !(&toh).is_powered().await?;
    // Ask symbiosis which TOH is attached, if any.
    match (&toh).detect().await {
        // A TOH is attached and identified.
        Ok(Some(info)) => {
            // "led-controller" comes from the TOH configs (blinker.yaml for the Inari Blue).
            match info.extra.get("led-controller") {
                // The Inari Blue: carry on.
                Some(ExtraValue::Text(controller)) if controller == "aw2023" => {}
                // Some other LED chip: we do not know how to drive it.
                Some(ExtraValue::Text(controller)) => {
                    return Err(format!("Unknown controller: {controller}").into())
                }
                // No LED controller at all (The Orange, Kaamos Black, Snow White…).
                _ => return Err("This is not the TOH we are looking for".into()),
            }
            // Leave power on while the light is on.
            let leave_powered = matches!(action, Action::On { .. });
            // The library makes sure we use the borrowing interface correctly here.
            if let Err(error) = toh
                // Borrow the I²C bus from symbiosis, then give it back when done.
                .access_i2c_dev_with_power(leave_powered, move |dev| {
                    // The work to do with the borrowed bus, as a pinned future.
                    Box::pin(async move {
                        // Wrap the bus in our LED chip driver.
                        let mut ctrl = LedController::new(dev);
                        // Freshly powered chip: give it half a second, as blinker does.
                        if needs_power_on {
                            sleep(Duration::from_millis(500)).await;
                        }
                        // Run the requested action on the chip.
                        match action {
                            // Steady light on one LED.
                            Action::On {
                                brightness,
                                current,
                                led,
                            } => ctrl.start_constant(brightness, current, led).await,
                            // Chip off.
                            Action::Off => ctrl.turn_led_off(),
                        }
                    })
                })
                // Outer `?`: borrowing failed (D-Bus, permissions…).
                .await?
            {
                // Inner error: the chip did not accept the register writes.
                Err(format!("Failed to set LED: {error}").into())
            } else {
                // All good.
                Ok(())
            }
        }
        // No TOH attached, or one symbiosis does not recognise.
        Ok(None) => Err("TOH is not present or is of unsupported type".into()),
        // Symbiosis is not on D-Bus.
        Err(FetchingInfoError::DBus(DBusError::ServiceUnknown(..))) => {
            Err("TOH daemon is not running".into())
        }
        // Any other D-Bus problem.
        Err(FetchingInfoError::DBus(error)) => Err(format!("D-Bus error: {:?}", error).into()),
        // TOH info that could not be decoded.
        Err(FetchingInfoError::Conversion(error)) => {
            Err(format!("Conversion error: {error}").into())
        }
    }
}

/// Activates or deactivates the front LED pattern in MCE.
async fn front_led(active: bool) -> Result<(), BoxError> {
    // MCE (the device state daemon) listens on the system bus.
    let connection = zbus::Connection::system().await?;
    // Pick the MCE method name.
    let method = if active {
        // Ask MCE to show our pattern (when its conditions are met).
        "req_led_pattern_activate"
    } else {
        // Ask MCE to stop showing it.
        "req_led_pattern_deactivate"
    };
    // Call the method, with the pattern name as its only argument.
    connection
        .call_method(
            // MCE's service name.
            Some("com.nokia.mce"),
            // MCE's request object.
            "/com/nokia/mce/request",
            // MCE's request interface.
            Some("com.nokia.mce.request"),
            // Activate or deactivate.
            method,
            // The pattern name, as a one-element tuple (a D-Bus string).
            &(FRONT_PATTERN,),
        )
        .await?;
    // Done.
    Ok(())
}

/// Front LED errors are reported but never stop the TOH light.
async fn front_led_or_warn(active: bool) {
    // Try, and only print the error if it fails.
    if let Err(error) = front_led(active).await {
        eprintln!("Front LED: {error}");
    }
}

/// Follows MCE display state: light on when the display is off, off otherwise.
async fn watch(brightness: u8, current: u8, led: u8) -> Result<(), BoxError> {
    // MCE's display signals are on the system bus.
    let connection = zbus::Connection::system().await?;

    // Subscribe before reading the current state, so no change is missed.
    let rule = zbus::MatchRule::builder()
        // Only signals…
        .msg_type(zbus::message::Type::Signal)
        // …from MCE's signal interface…
        .interface("com.nokia.mce.signal")?
        // …announcing display changes ("on", "dimmed", "off").
        .member("display_status_ind")?
        .build();
    // A stream that yields each matching signal.
    let mut signals = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;

    // Ask MCE for the display state right now.
    let reply = connection
        .call_method(
            Some("com.nokia.mce"),
            "/com/nokia/mce/request",
            Some("com.nokia.mce.request"),
            "get_display_status",
            // No arguments.
            &(),
        )
        .await?;
    // The answer is a single string: "on", "dimmed" or "off".
    let mut status: String = reply.body().deserialize()?;

    // Front LED: activate once; MCE shows it only while the display is off.
    front_led_or_warn(true).await;

    // None until the first successful update.
    let mut lit: Option<bool> = None;
    // Runs until the service is stopped.
    loop {
        // "dimmed" counts as on: only a fully off display lights the TOH.
        let off = status == "off";
        // Only touch the chip when the wanted state changes.
        if lit != Some(off) {
            // Display off: light up; display on: switch off.
            let action = if off {
                Action::On {
                    brightness,
                    current,
                    led,
                }
            } else {
                Action::Off
            };
            // Apply it to the TOH.
            match apply(action).await {
                // Success: log it and remember the new state.
                Ok(()) => {
                    println!("Display {status}: light {}", if off { "on" } else { "off" });
                    lit = Some(off);
                }
                // Failure (e.g. no TOH attached): log it, retry on the next change.
                Err(error) => eprintln!("Display {status}: {error}"),
            }
        }

        // Wait for the next display signal.
        match signals.next().await {
            // A signal arrived: read the new state from its body.
            Some(Ok(message)) => {
                if let Ok(new_status) = message.body().deserialize::<String>() {
                    status = new_status;
                }
            }
            // A broken message: log it and keep listening.
            Some(Err(error)) => eprintln!("Signal error: {error}"),
            // The stream ended (D-Bus connection lost): exit, systemd restarts us.
            None => return Err("Display signal stream closed".into()),
        }
    }
}

// Single-threaded async runtime: enough for one D-Bus loop, and light.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BoxError> {
    // Parse the command line.
    let args: Arguments = argh::from_env();

    // Run the chosen command.
    match args.command {
        // `pharos on`: front pattern on, TOH LED on.
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
        // `pharos off`: front pattern off, TOH LED off (also run when the service stops).
        Command::Off(_) => {
            println!("Turning LED off");
            front_led_or_warn(false).await;
            apply(Action::Off).await
        }
        // `pharos watch`: follow the display until stopped.
        Command::Watch(watch_args) => {
            watch(watch_args.brightness, watch_args.current, watch_args.led).await
        }
    }
}
