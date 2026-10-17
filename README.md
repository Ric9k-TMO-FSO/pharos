# Pharos

Inspired from the original Jolla Inari Blue TOH LED toggle button and system.

Find your phone in the night.

A very dim, steady light while the display is off, to find our Sailfish OS
phone in the dark.

- Front notification LED, dimmed through an MCE pattern (`PatternPharos`).
- With the Inari Blue TOH attached: its middle LED, in constant mode.
- A top menu switch to turn it on and off.

This project was created with heavy LLM use, and some human brain and knowledge
where possible.

Please note: this has not yet been tested over the long term.
You use it at your own risk, particularly in the event of data loss or
damage to the software or hardware.


## Compatibility and testing

- Briefly tested on the Jolla Phone (2026) with Sailfish OS 5.2.0.20 and the
  Inari Blue TOH.
- The TOH LED part only works on the Jolla Phone (2026) with the Inari Blue
  TOH: it needs the TOH connector, Jolla's symbiosis service and the Inari
  Blue LED controller.
- The front LED part uses a standard MCE LED pattern, so it may work on other
  Sailfish OS devices with a notification LED, but it has not been tested on
  any other device.

## Build

With the Sailfish SDK, from the project root (Rust and the translation tools
come from the SDK target):

    sfdk build

If rustc fails with `No such device (os error 19)`, the project folder is
shared with the build engine in a way rustc cannot use. Build inside the
engine instead, then copy the package back:

    sfdk engine exec bash -c 'rsync -a --delete --exclude target --exclude cargo-home --exclude RPMS ~/path/to/pharos/ ~/pharos/ && cd ~/pharos && mb2 -t <target> build && cp -r RPMS ~/path/to/pharos/'

The package is written to `RPMS/`.

## Install

    pkcon install-local pharos-*.rpm    # as root (devel-su)

Then add **Pharos** to the top menu in Settings and switch it on.

## Files

- `src/` – `pharos` program (`on`, `off`, `watch`)
- `data/` – user service, symbiosis TOH config, MCE pattern, settings entry
- `qml/` – top menu switch
- `icons/` – icon source (`icon-m-pharos.svg`) and PNGs per density
- `translations/` – `.ts` sources
- `rpm/pharos.spec`
- `NOTES.md` – design notes

## Licence

BSD-3-Clause, see `LICENSE`.

## Credits

Pharos is an independent project. It is not affiliated with, endorsed by or
supported by Jolla. "Jolla", "Sailfish OS" and "Inari Blue" are used only to
name the hardware and software Pharos works with.

Pharos builds on code that Jolla published under the BSD-3-Clause licence,
with thanks. Jolla's copyright notice is kept in the files derived from it:

- `src/main.rs`, `src/controller.rs`: derived from the `blinker` example of
  [symbiosis](https://github.com/sailfishos/symbiosis) (AW2023 register map,
  enable sequence, TOH detection and I²C borrowing).
- `qml/EnableSwitch.qml`: derived from the Inari Blue top menu switch.
- `rpm/pharos.spec`: Rust build section derived from symbiosis' spec.

Pharos also depends on the symbiosis library (BSD-3-Clause) at build time.
All other files are original work.
