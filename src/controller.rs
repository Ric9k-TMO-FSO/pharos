// SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
//
// SPDX-License-Identifier: BSD-3-Clause

//! AW2023 LED controller, steady (direct mode) light.
//!
//! Register map and enable sequence taken from the blinker example.

use num_enum::IntoPrimitive;
use std::io::{Error, Read, Result, Write};
use std::time::Duration;
use symbiosis::i2cdev::I2cDev;
use tokio::time::sleep;

const AW2023_TARGET_ADDRESS: u32 = 0x45;

#[derive(IntoPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
enum AW2023 {
    Gcr1 = 0x01,
    Gcr2 = 0x04,
    LCtr = 0x30,
    LCfg0 = 0x31,
    LCfg1 = 0x32,
    LCfg2 = 0x33,
    Pwm0 = 0x34,
    Pwm1 = 0x35,
    Pwm2 = 0x36,
}

pub struct LedController<'dev> {
    dev: &'dev mut I2cDev,
}

impl<'dev> LedController<'dev> {
    pub fn new(dev: &'dev mut I2cDev) -> Self {
        Self { dev }
    }

    fn read_register(&mut self, register: AW2023) -> Result<u8> {
        self.dev.set_target_address(AW2023_TARGET_ADDRESS)?;
        self.dev.write_all(&[register.into()])?;
        let mut output = [0u8; 1];
        self.dev.read_exact(&mut output)?;
        Ok(output[0])
    }

    fn write_register(&mut self, register: impl Into<u8>, value: u8) -> Result<()> {
        self.dev.set_target_address(AW2023_TARGET_ADDRESS)?;
        self.dev.write_all(&[register.into(), value])?;
        Ok(())
    }

    async fn turn_led_on(&mut self) -> Result<()> {
        for _ in 0..3 {
            // GCR1.CHIPEN = 1
            self.write_register(AW2023::Gcr1, 0x01)?;
            if (self.read_register(AW2023::Gcr1)? & 0x01) > 0 {
                return Ok(());
            }
            sleep(Duration::from_millis(10)).await;
        }
        Err(Error::other("failed after three retries"))
    }

    pub fn turn_led_off(&mut self) -> Result<()> {
        // GCR1.CHIPEN = 0
        self.write_register(AW2023::Gcr1, 0x00)
    }

    /// Steady light on a single LED: LCFGx.MD = 0 (direct mode), so PWMx sets a
    /// constant brightness with no pattern cycle to restart.
    pub async fn start_constant(&mut self, brightness: u8, current: u8, led: u8) -> Result<()> {
        if brightness == 0 {
            return self.turn_led_off();
        }
        if led > 2 {
            return Err(Error::other("LED must be 0, 1 or 2"));
        }
        // LCFGx: CUR in the low 4 bits, MD (bit 4) and SYNC (bit 7) left at 0.
        let lcfg = current.min(15);
        let pwm_register = u8::from(AW2023::Pwm0) + led;

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
            .and_then(|_| self.write_register(pwm_register, brightness))
            // LCTR.LEx = 1 for the chosen LED only
            .and_then(|_| self.write_register(AW2023::LCtr, 1 << led))
            .or_else(|_| self.turn_led_off())
    }
}
