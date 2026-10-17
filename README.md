# Pharos

A very dim, steady light while the display is off, to find your Sailfish OS
phone in the dark.

- Front notification LED, dimmed through an MCE pattern (`PatternPharos`).
- With the Inari Blue TOH attached: its middle LED, in constant mode.
- A top menu switch to turn it on and off.

## Build

With the Sailfish SDK (Rust comes from the target), from the project root:

    sfdk build

If the project sits on a folder shared with the build engine and rustc fails
with `No such device (os error 19)`, build inside the engine instead:

    sfdk engine exec bash -c 'rsync -a --delete --exclude target --exclude cargo-home --exclude RPMS ~/path/to/pharos/ ~/pharos/ && cd ~/pharos && mb2 -t <target> build && cp -r RPMS ~/path/to/pharos/'

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

BSD-3-Clause. Partly derived from Jolla's symbiosis `blinker` example.
