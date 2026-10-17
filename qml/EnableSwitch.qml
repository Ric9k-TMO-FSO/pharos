// SPDX-FileCopyrightText: 2026 Jolla Mobile Ltd
// SPDX-FileCopyrightText: 2026 Ric9k
//
// SPDX-License-Identifier: BSD-3-Clause
//
// Top menu switch for pharos, derived from Jolla's Inari Blue switch
// (/usr/share/jolla-settings/pages/inari_blue/EnableSwitch.qml, BSD-3-Clause).

import QtQuick 2.6
import Sailfish.Silica 1.0
import Nemo.DBus 2.0
import com.jolla.settings 1.0

SettingsToggle {
    id: root

    //: Top menu switch: keeps a dim light on while the display is off.
    //: Pharos is the application name; translate or keep as it suits the language.
    //% "Pharos"
    name: qsTrId("pharos-la-name")
    icon.source: "image://theme/icon-m-pharos"
    onToggled: systemd.toggle()
    checked: false

    DBusInterface {
        id: systemd

        bus: DBus.SessionBus
        service: "org.freedesktop.systemd1"
        iface: "org.freedesktop.systemd1.Manager"
        path: "/org/freedesktop/systemd1"

        signalsEnabled: true

        function update() {
            call('GetUnitFileState', "pharos.service", function(state) {
                root.checked = state == "enabled"
            })
        }

        function toggle() {
            if (root.checked) {
                // To turn off: stop now, and do not start at next boot
                call('StopUnit', ['pharos.service', 'replace'], function(reply) {
                    typedCall('DisableUnitFiles', [
                        { 'type': 'as', 'value': ['pharos.service'] },
                        { 'type': 'b', 'value': false }
                    ], function(reply) { call('Reload', undefined) })
                })
            } else {
                // To turn on: Jolla's breathing effect gives the TOH LED up,
                // then pharos is enabled for next boots and started now
                call('StopUnit', ['toh-leds.service', 'replace'], function(reply) {
                    typedCall('MaskUnitFiles', [
                        { 'type': 'as', 'value': ['toh-leds.service'] },
                        { 'type': 'b', 'value': false },
                        { 'type': 'b', 'value': false }
                    ], function(reply) {
                        typedCall('EnableUnitFiles', [
                            { 'type': 'as', 'value': ['pharos.service'] },
                            { 'type': 'b', 'value': false },
                            { 'type': 'b', 'value': false }
                        ], function(reply) {
                            call('Reload', undefined, function(reply) {
                                call('StartUnit', ['pharos.service', 'replace'])
                            })
                        })
                    })
                })
            }
        }

        function subscribe() {
            call('Subscribe', undefined)
        }

        function unitFilesChanged() {
            update()
        }

        Component.onCompleted: {
            subscribe()
            update()
        }
    }
}
