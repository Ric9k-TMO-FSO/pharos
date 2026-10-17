# Pharos – design notes

*Written by the LLM.*

Goal: a very dim, steady light while the display is off, to find the phone in the dark.

Name: Pharos, the lighthouse of Alexandria, origin of the word for lighthouse in
many languages (phare, faro…). Kept untranslated as a proper name.

## Decision: front LED + one TOH LED

- Front notification LED, dimmed by MCE.
- One of the three Inari Blue TOH LEDs (`--led 0|1|2`, default 1), in steady
  direct mode. One LED is enough at night and costs a third of three.
- Both stay on while the display is off, day or night: no night detection.
- Works without the TOH: the front LED is handled by MCE; the TOH LED is
  set on each display change only when the TOH is present.

## Architecture

- `pharos watch`: user service started with the session
  (`pharos.service`, `WantedBy=post-user-session.target`).
  - Activates the MCE pattern `PatternPharos` once at start; MCE shows it
    only while the display is off.
  - Listens to MCE `display_status_ind`: display off → TOH LED on,
    display on → TOH LED off.
- `pharos off` (run on service stop): deactivates the MCE pattern and
  turns the TOH LED off.
- Top menu switch (`EnableSwitch.qml`, `pharos.json`): enables/starts or
  stops/disables `pharos.service`; when turned on, it also stops and masks
  Jolla's `toh-leds.service` (breathing effect), which would fight for the TOH LED.
- `50-pharos.yaml` in `/usr/share/tohd-1/tohs/0001/0004/`: lets
  `/usr/libexec/pharos/pharos` borrow the TOH I²C bus from symbiosis.

## Front LED (MCE)

Patterns are defined in `/etc/mce/*.ini`, read in alphabetical order; a later
file overrides only the keys it redefines. Pharos ships its own file:

```ini
# /etc/mce/90-pharos.ini
[LEDPatternHybris]
# priority;screen-on;timeout;on-ms;off-ms;RGB
PatternPharos=200;0;0;0;0;11042b
```

- RGB `11042b`: dim violet (R 17, G 4, B 43 out of 255).
- Screen-on `0`: shown only while the display is off.
- Priority 200: above the base pattern (254), below notifications (~30),
  so notifications still take over the LED.
- Dimmed colours only work for steady patterns: with blinking, MCE only
  supports full-intensity colours.
- Pattern definitions are root-only (`/etc/mce`); activating a pattern over
  D-Bus (`req_led_pattern_activate`) works as a normal user.
- MCE reads its config at start: `systemctl restart mce` after a change.

## TOH LED (AW2023)

- Controller: Awinic AW2023 at I²C address 0x45 (symbiosis `examples/blinker`).
- Pattern mode (LCFG MD=1) restarts its cycle every ~7 s (max hold time
  31.25 ms × 15²) and flickers at low PWM.
- Direct mode (MD=0): PWM sets a steady brightness, at any level.
  All three LCFG registers are reset to direct mode with SYNC cleared, since
  Jolla's breathing effect leaves LCFG0 in pattern + sync mode.
- Only the chosen LED is enabled in LCTR; the other PWM registers are zeroed.
- No root needed: symbiosis lends the I²C file descriptor
  (`BorrowI2cDevAccessWithPower`) to executables listed in a TOH config
  under `/usr/share/tohd-1/tohs/VID/PID/*.yaml` (access lists are merged).
- Inari Blue: vendor 0001, product 0004.

## Night detection: not needed

Considered, then dropped.

- Sampling the light sensor only when the display turns off fails in the
  common case: phone put down in daylight, night falls later.
- Detecting nightfall with the display off means waking the CPU from deep
  sleep (keepalive / DSME timers), e.g. every 10–15 min. Each wake-up costs
  a few seconds at 100+ mA, so a few mAh per day, plus false nights in a
  pocket or bag.
- Clock-based night (fixed hours or sunset) would need only one wake-up per
  night, but is unnecessary once the light is cheap enough to stay on.

## Battery estimate

- One dim LED (~1 % of 30 mA) draws about 0.3 mA; the dimmed front LED is
  of the same order. Together roughly 0.5–1 mA: under 25 mAh per day,
  below 1 % of the battery.
- This is comparable to or less than periodic wake-ups, so an always-on dim
  light while the display is off is the simpler choice.
- Unknown: the TOH 5 V supply stays on while the TOH LED is lit
  (`leave_powered`), and its own consumption is not known.
- To check: compare overnight battery drain with pharos on and off,
  TOH attached, without charging. If the TOH supply costs too much, light the
  TOH LED only at night (clock-based, one wake-up per night).

## To do

- Overnight battery measurement, pharos on and off.
