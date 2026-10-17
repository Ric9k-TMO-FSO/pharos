# SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
# SPDX-FileCopyrightText: 2026 Ric9k
#
# SPDX-License-Identifier: BSD-3-Clause
#
# Build with the Sailfish SDK from the project root: sfdk build
# (or mb2 -t <target> build inside the build engine).
# The Rust build section is derived from Jolla's rpm/symbiosis.spec.

# Hardened linking (full RELRO), as in symbiosis.
%global rustflags -Clink-arg=-Wl,-z,relro,-z,now

# Package name.
Name:           pharos
# Pharos version (keep in sync with Cargo.toml).
Version:        0.1.0
# Package build number for this version.
Release:        1
# One-line description.
Summary:        Dim lights while the display is off, to find the phone in the dark
# Licence of the package contents.
License:        BSD-3-Clause
# Project home.
URL:            https://github.com/Ric9k-TMO-FSO/pharos

# The Jolla Phone (2026) is 64-bit ARM only.
ExclusiveArch:  aarch64

# Rust toolchain from the SDK target (1.75 in Sailfish OS 5.x).
BuildRequires:  cargo >= 1.75.0
BuildRequires:  rust >= 1.75.0
BuildRequires:  rust-std-static >= 1.75.0
# lrelease, to compile the translations.
BuildRequires:  qt5-qttools-linguist
# Jolla's TOH daemon: lends the TOH I2C bus.
Requires:       symbiosis
# Device state daemon: front LED patterns and display signals.
Requires:       mce
# The install scripts call systemctl.
Requires(post): systemd
Requires(postun): systemd

# Where user services, theme icons and Inari Blue TOH configs go.
%define userunitdir %{_prefix}/lib/systemd/user
%define themedir %{_datadir}/themes/sailfish-default/silica
%define inaridir %{_datadir}/tohd-1/tohs/0001/0004

%description
Find your phone in the night.
Pharos keeps a very dim, steady light while the display is off: the front
notification LED (through an MCE pattern) and, when the Inari Blue TOH is
attached, one of its LEDs in constant mode. A top menu switch turns it on
and off.
Only one LED of the Inari Blue TOH is used, at about 1 %, to use as little
battery as possible.
There is no automatic switch on when the ambient light is low: detecting
nightfall with the display off would wake the phone from deep sleep, which
could use more battery than the LEDs at minimum power. Theory, to be
confirmed.
Briefly tested on the Jolla Phone (2026). The TOH LED part only works on that
phone with the Inari Blue TOH; the front LED part is untested elsewhere.

%build
# Cross-compiling under SB2: tell rust which arch to emit (see symbiosis.spec)
export SB2_RUST_TARGET_TRIPLE=aarch64-unknown-linux-gnu
export SB2_RUST_EXECVP_SHIM="/usr/bin/env LD_PRELOAD=/usr/lib/libsb2/libsb2.so.1 /usr/bin/env"
export SB2_RUST_USE_REAL_EXECVP=Yes
export SB2_RUST_USE_REAL_FN=Yes
# Linker flags defined at the top.
export RUSTFLAGS="%{rustflags}"
# Keep the downloaded crates inside the build folder.
export CARGO_HOME=`pwd`/cargo-home/
# Release build; one job and the locked dependency versions, as in symbiosis.
cargo build -j1 --locked --target $SB2_RUST_TARGET_TRIPLE --release

# Translations: engineering English and languages, id based
# Output folder for the compiled translations.
mkdir -p qm
# Engineering English: the fallback texts.
lrelease -idbased translations/pharos_eng_en.ts -qm qm/pharos_eng_en.qm
# One compiled file per language, e.g. pharos-fr.qm.
for ts in translations/pharos-*.ts; do
    lrelease -idbased "$ts" -qm "qm/$(basename "$ts" .ts).qm"
done

%install
%define rustbuilddir target/aarch64-unknown-linux-gnu/release

# Program and services
# The program, in its own libexec folder.
install -D -m0755 %{rustbuilddir}/%{name} %{buildroot}%{_libexecdir}/%{name}/%{name}
# User service, started with the session when enabled.
install -D -m0644 data/pharos.service %{buildroot}%{userunitdir}/pharos.service
# Symbiosis config: I2C access for the program, Inari Blue only.
install -D -m0644 data/50-pharos.yaml %{buildroot}%{inaridir}/50-pharos.yaml
# MCE front LED pattern.
install -D -m0644 data/90-pharos.ini %{buildroot}%{_sysconfdir}/mce/90-pharos.ini

# Top menu switch
# Settings entry, which makes the switch available in the top menu.
install -D -m0644 data/pharos.json %{buildroot}%{_datadir}/jolla-settings/entries/pharos.json
# The switch itself.
install -D -m0644 qml/EnableSwitch.qml %{buildroot}%{_datadir}/jolla-settings/pages/pharos/EnableSwitch.qml

# Icon, one per display density
for z in z1.0 z1.25 z1.5 z1.75 z2.0 z2.5; do
    install -D -m0644 icons/$z/icon-m-pharos.png \
        %{buildroot}%{themedir}/$z/icons-monochrome/icon-m-pharos.png
done

# Translations
# Compiled translations, where Qt looks for them.
install -d %{buildroot}%{_datadir}/translations
install -m0644 qm/*.qm %{buildroot}%{_datadir}/translations/

%post
# Symbiosis grants I2C access from its TOH configs, MCE reads LED patterns at start
systemctl try-restart symbiosis.service || :
systemctl try-restart mce.service || :
# Let the user systemd see the new pharos.service.
systemctl-user daemon-reload || :

%preun
# Full removal only (not an upgrade): stop pharos and stop starting it.
if [ $1 -eq 0 ]; then
    systemctl-user disable --now pharos.service || :
fi

%postun
# Full removal only: drop the I2C access and the MCE pattern, forget the service.
if [ $1 -eq 0 ]; then
    systemctl try-restart symbiosis.service || :
    systemctl try-restart mce.service || :
    systemctl-user daemon-reload || :
fi

%files
# Files owned by the package.
%license LICENSE
%{_libexecdir}/%{name}
%{userunitdir}/pharos.service
%{inaridir}/50-pharos.yaml
# Local edits of the LED pattern are kept on upgrade.
%config(noreplace) %{_sysconfdir}/mce/90-pharos.ini
%{_datadir}/jolla-settings/entries/pharos.json
%{_datadir}/jolla-settings/pages/pharos
%{themedir}/*/icons-monochrome/icon-m-pharos.png
%{_datadir}/translations/pharos*.qm
