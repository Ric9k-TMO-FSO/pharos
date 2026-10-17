# SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
#
# SPDX-License-Identifier: BSD-3-Clause
#
# Build with the Sailfish SDK from the project root: sfdk build
# (or mb2 -t <target> build inside the build engine).

%global rustflags -Clink-arg=-Wl,-z,relro,-z,now

Name:           pharos
Version:        0.1.0
Release:        1
Summary:        Dim lights while the display is off, to find the phone in the dark
License:        BSD-3-Clause
URL:            https://github.com/YOUR_ACCOUNT/pharos

ExclusiveArch:  aarch64

BuildRequires:  cargo >= 1.75.0
BuildRequires:  rust >= 1.75.0
BuildRequires:  rust-std-static >= 1.75.0
BuildRequires:  qt5-qttools-linguist
Requires:       symbiosis
Requires:       mce
Requires(post): systemd
Requires(postun): systemd

%define userunitdir %{_prefix}/lib/systemd/user
%define themedir %{_datadir}/themes/sailfish-default/silica
%define inaridir %{_datadir}/tohd-1/tohs/0001/0004

%description
Pharos keeps a very dim, steady light while the display is off: the front
notification LED (through an MCE pattern) and, when the Inari Blue TOH is
attached, one of its LEDs in constant mode. A top menu switch turns it on
and off.

%build
# Cross-compiling under SB2: tell rust which arch to emit (see symbiosis.spec)
export SB2_RUST_TARGET_TRIPLE=aarch64-unknown-linux-gnu
export SB2_RUST_EXECVP_SHIM="/usr/bin/env LD_PRELOAD=/usr/lib/libsb2/libsb2.so.1 /usr/bin/env"
export SB2_RUST_USE_REAL_EXECVP=Yes
export SB2_RUST_USE_REAL_FN=Yes
export RUSTFLAGS="%{rustflags}"
export CARGO_HOME=`pwd`/cargo-home/
cargo build -j1 --locked --target $SB2_RUST_TARGET_TRIPLE --release

# Translations: engineering English and languages, id based
mkdir -p qm
lrelease -idbased translations/pharos_eng_en.ts -qm qm/pharos_eng_en.qm
for ts in translations/pharos-*.ts; do
    lrelease -idbased "$ts" -qm "qm/$(basename "$ts" .ts).qm"
done

%install
%define rustbuilddir target/aarch64-unknown-linux-gnu/release

# Program and services
install -D -m0755 %{rustbuilddir}/%{name} %{buildroot}%{_libexecdir}/%{name}/%{name}
install -D -m0644 data/pharos.service %{buildroot}%{userunitdir}/pharos.service
install -D -m0644 data/50-pharos.yaml %{buildroot}%{inaridir}/50-pharos.yaml
install -D -m0644 data/90-pharos.ini %{buildroot}%{_sysconfdir}/mce/90-pharos.ini

# Top menu switch
install -D -m0644 data/pharos.json %{buildroot}%{_datadir}/jolla-settings/entries/pharos.json
install -D -m0644 qml/EnableSwitch.qml %{buildroot}%{_datadir}/jolla-settings/pages/pharos/EnableSwitch.qml

# Icon, one per display density
for z in z1.0 z1.25 z1.5 z1.75 z2.0 z2.5; do
    install -D -m0644 icons/$z/icon-m-pharos.png \
        %{buildroot}%{themedir}/$z/icons-monochrome/icon-m-pharos.png
done

# Translations
install -d %{buildroot}%{_datadir}/translations
install -m0644 qm/*.qm %{buildroot}%{_datadir}/translations/

%post
# Symbiosis grants I2C access from its TOH configs, MCE reads LED patterns at start
systemctl try-restart symbiosis.service || :
systemctl try-restart mce.service || :
systemctl-user daemon-reload || :

%preun
if [ $1 -eq 0 ]; then
    systemctl-user disable --now pharos.service || :
fi

%postun
if [ $1 -eq 0 ]; then
    systemctl try-restart symbiosis.service || :
    systemctl try-restart mce.service || :
    systemctl-user daemon-reload || :
fi

%files
%license LICENSE
%{_libexecdir}/%{name}
%{userunitdir}/pharos.service
%{inaridir}/50-pharos.yaml
%config(noreplace) %{_sysconfdir}/mce/90-pharos.ini
%{_datadir}/jolla-settings/entries/pharos.json
%{_datadir}/jolla-settings/pages/pharos
%{themedir}/*/icons-monochrome/icon-m-pharos.png
%{_datadir}/translations/pharos*.qm
