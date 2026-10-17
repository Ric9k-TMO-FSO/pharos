// SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
// SPDX-FileCopyrightText: 2026 Ric9k
//
// SPDX-License-Identifier: BSD-3-Clause

//! AW2023 LED controller, steady (direct mode) light.
//!
//! Register map and enable sequence taken from the blinker example of Jolla's
//! symbiosis (BSD-3-Clause), examples/blinker/controller.rs.

// Turns the register enum below into plain bytes.
use num_enum::IntoPrimitive;
// I/O error and result types, and the read/write traits used on the bus.
use std::io::{Error, Read, Result, Write};
// Time spans, for the retry delay.
use std::time::Duration;
// The I²C bus device lent by symbiosis.
use symbiosis::i2cdev::I2cDev;
// Asynchronous sleep.
use tokio::time::sleep;

// I²C address of the AW2023 LED chip in the Inari Blue TOH.
const AW2023_TARGET_ADDRESS: u32 = 0x45;

// AW2023 registers used here (names from the chip's datasheet, as in blinker).
#[derive(IntoPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
enum AW2023 {
    // Global control 1: CHIPEN bit switches the whole chip on or off.
    Gcr1 = 0x01,
    // Global control 2: IMAX, the maximum LED current for all channels.
    Gcr2 = 0x04,
    // LED control: one enable bit per LED (LE0, LE1, LE2).
    LCtr = 0x30,
    // LED 0 config: current step (CUR), pattern mode (MD), sync (SYNC).
    LCfg0 = 0x31,
    // LED 1 config.
    LCfg1 = 0x32,
    // LED 2 config.
    LCfg2 = 0x33,
    // LED 0 brightness (PWM duty, 0-255).
    Pwm0 = 0x34,
    // LED 1 brightness.
    Pwm1 = 0x35,
    // LED 2 brightness.
    Pwm2 = 0x36,
}

// Driver holding the borrowed bus for the time of one operation.
pub struct LedController<'dev> {
    // The I²C bus; the lifetime ties the driver to the borrow.
    dev: &'dev mut I2cDev,
}

impl<'dev> LedController<'dev> {
    // Wrap a borrowed bus.
    pub fn new(dev: &'dev mut I2cDev) -> Self {
        Self { dev }
    }

    // Read one register: write its address, then read one byte back.
    fn read_register(&mut self, register: AW2023) -> Result<u8> {
        // Talk to the LED chip.
        self.dev.set_target_address(AW2023_TARGET_ADDRESS)?;
        // Select the register.
        self.dev.write_all(&[register.into()])?;
        // One-byte buffer for the answer.
        let mut output = [0u8; 1];
        // Read the register value.
        self.dev.read_exact(&mut output)?;
        // Return it.
        Ok(output[0])
    }

    // Write one register: address byte, then value byte, in a single I²C write.
    // Takes a raw byte too, for the computed per-LED PWM register.
    fn write_register(&mut self, register: impl Into<u8>, value: u8) -> Result<()> {
        // Talk to the LED chip.
        self.dev.set_target_address(AW2023_TARGET_ADDRESS)?;
        // Register address, then its new value.
        self.dev.write_all(&[register.into(), value])?;
        // Done.
        Ok(())
    }

    // Switch the chip on, checking it really started (as blinker does).
    async fn turn_led_on(&mut self) -> Result<()> {
        // Up to three attempts.
        for _ in 0..3 {
            // GCR1.CHIPEN = 1
            self.write_register(AW2023::Gcr1, 0x01)?;
            // Read it back: is CHIPEN set?
            if (self.read_register(AW2023::Gcr1)? & 0x01) > 0 {
                // Yes: the chip is on.
                return Ok(());
            }
            // Not yet: wait 10 ms and retry.
            sleep(Duration::from_millis(10)).await;
        }
        // Still off after three tries.
        Err(Error::other("failed after three retries"))
    }

    // Switch the whole chip off: all LEDs go dark.
    pub fn turn_led_off(&mut self) -> Result<()> {
        // GCR1.CHIPEN = 0
        self.write_register(AW2023::Gcr1, 0x00)
    }

    /// Steady light on a single LED: LCFGx.MD = 0 (direct mode), so PWMx sets a
    /// constant brightness with no pattern cycle to restart.
    pub async fn start_constant(&mut self, brightness: u8, current: u8, led: u8) -> Result<()> {
        // Brightness 0 means off.
        if brightness == 0 {
            return self.turn_led_off();
        }
        // Only three LEDs: 0, 1 and 2.
        if led > 2 {
            return Err(Error::other("LED must be 0, 1 or 2"));
        }
        // LCFGx: CUR in the low 4 bits, MD (bit 4) and SYNC (bit 7) left at 0.
        let lcfg = current.min(15);
        // PWM registers follow each other: PWM0, PWM1, PWM2.
        let pwm_register = u8::from(AW2023::Pwm0) + led;

        // Chip on, then the register writes in order; any failure skips the rest.
        self.turn_led_on()
            .await
            // GCR2.IMAX = 1 (30 mA), as in the blinker example
            .and_then(|_| self.write_register(AW2023::Gcr2, 0x01))
            // All channels to direct mode, with SYNC cleared: Jolla's breathing
            // effect leaves LCFG0 in pattern + sync mode, which would drive the others.
            .and_then(|_| self.write_register(AW2023::LCfg0, lcfg))
            .and_then(|_| self.write_register(AW2023::LCfg1, lcfg))
            .and_then(|_| self.write_register(AW2023::LCfg2, lcfg))
            // Clear the other channels so nothing is left over from a previous effect
            .and_then(|_| self.write_register(AW2023::Pwm0, 0))
            .and_then(|_| self.write_register(AW2023::Pwm1, 0))
            .and_then(|_| self.write_register(AW2023::Pwm2, 0))
            // Brightness of the chosen LED.
            .and_then(|_| self.write_register(pwm_register, brightness))
            // LCTR.LEx = 1 for the chosen LED only
            .and_then(|_| self.write_register(AW2023::LCtr, 1 << led))
            // On any failure, switch the chip off rather than leave it half set.
            // As in blinker, the original error is then dropped if switching off works.
            .or_else(|_| self.turn_led_off())
    }
}
