// SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
// SPDX-FileCopyrightText: 2026 Ric9k
//
// SPDX-License-Identifier: BSD-3-Clause
//
// Top menu switch for pharos, derived from Jolla's Inari Blue switch
// (/usr/share/jolla-settings/pages/inari_blue/EnableSwitch.qml, BSD-3-Clause).

// Basic QML types.
import QtQuick 2.6
// Sailfish UI components.
import Sailfish.Silica 1.0
// D-Bus access from QML.
import Nemo.DBus 2.0
// SettingsToggle, the round top menu / settings switch.
import com.jolla.settings 1.0

// The switch itself.
SettingsToggle {
    // Name used below to update the switch state.
    id: root

    // Translator notes (//:) and default text (//%) must stay right above qsTrId.
    //: Top menu switch: keeps a dim light on while the display is off.
    //: Pharos is the application name; translate or keep as it suits the language.
    //% "Pharos"
    name: qsTrId("pharos-la-name")
    // Theme icon, installed per display density (icons-monochrome).
    icon.source: "image://theme/icon-m-pharos"
    // A tap on the switch turns pharos on or off.
    onToggled: systemd.toggle()
    // Off until update() has read the real state.
    checked: false

    // The user's systemd, which starts and stops pharos.service.
    DBusInterface {
        // Name used by onToggled above.
        id: systemd

        // User services live on the session bus.
        bus: DBus.SessionBus
        // systemd's D-Bus name.
        service: "org.freedesktop.systemd1"
        // Its manager interface (start, stop, enable…).
        iface: "org.freedesktop.systemd1.Manager"
        // Its manager object.
        path: "/org/freedesktop/systemd1"

        // Receive systemd signals, so unitFilesChanged() below is called.
        signalsEnabled: true

        // Read whether pharos is enabled, and show it on the switch.
        function update() {
            call('GetUnitFileState', "pharos.service", function(state) {
                // "enabled" = starts with the session = switch on.
                root.checked = state == "enabled"
            })
        }

        // Turn pharos on or off, depending on the current state.
        function toggle() {
            if (root.checked) {
                // To turn off: stop now, and do not start at next boot
                call('StopUnit', ['pharos.service', 'replace'], function(reply) {
                    // Remove the "start with the session" link.
                    typedCall('DisableUnitFiles', [
                        // Units to disable.
                        { 'type': 'as', 'value': ['pharos.service'] },
                        // false = permanently, not only until reboot.
                        { 'type': 'b', 'value': false }
                    // Make systemd re-read its unit files.
                    ], function(reply) { call('Reload', undefined) })
                })
            } else {
                // To turn on: Jolla's breathing effect gives the TOH LED up,
                // then pharos is enabled for next boots and started now
                call('StopUnit', ['toh-leds.service', 'replace'], function(reply) {
                    // Mask Jolla's breathing effect so symbiosis cannot restart it.
                    typedCall('MaskUnitFiles', [
                        // Units to mask.
                        { 'type': 'as', 'value': ['toh-leds.service'] },
                        // false = permanently, not only until reboot.
                        { 'type': 'b', 'value': false },
                        // false = do not overwrite an existing unit file.
                        { 'type': 'b', 'value': false }
                    ], function(reply) {
                        // Start pharos with every session from now on.
                        typedCall('EnableUnitFiles', [
                            // Units to enable.
                            { 'type': 'as', 'value': ['pharos.service'] },
                            // false = permanently, not only until reboot.
                            { 'type': 'b', 'value': false },
                            // false = do not overwrite existing links.
                            { 'type': 'b', 'value': false }
                        ], function(reply) {
                            // Re-read unit files, then start pharos now.
                            call('Reload', undefined, function(reply) {
                                call('StartUnit', ['pharos.service', 'replace'])
                            })
                        })
                    })
                })
            }
        }

        // Ask systemd to send us its signals.
        function subscribe() {
            call('Subscribe', undefined)
        }

        // systemd signal: unit files changed (enable, disable, mask…), maybe elsewhere.
        function unitFilesChanged() {
            // This is not very efficient but there doesn't seem to be a better way
            update()
        }

        // When the switch is created: listen to systemd, then show the real state.
        Component.onCompleted: {
            subscribe()
            update()
        }
    }
}
