# skatebuddy

Skatebuddy is a work-in-progress firmware for the [SMA Q3/Bangle.js 2 smart watch](https://banglejs.com/).

## Features
 - Automatic time synchronization (via gps)
 - Countdown timer
 - Stop watch
 - Heart rate monitor (accurate when holding arm still, during workout still hit-and-miss)
 - Workout tracker (hrm, gps; still work-in-progress)
 - Roughly 1 month of battery life
 - Persistent flash storage via [littlefs](https://github.com/littlefs-project/littlefs)

## Building/flashing

You'll need a debug-probe to connect to the middle two pins on the back of the watch.
See the official [documentation](https://www.espruino.com/Bangle.js2+Technical#swd).
One way to connect to these pins is to plug the charging cable into a female usb plug (e.g. by cutting a usb extension cord) and see which cables connect to the middle two pads.
Then attach these two (plus ground) to a debug probe, for example a [raspberrypi pico](https://github.com/raspberrypi/debugprobe).

Then compile and flash the firmware in one step using `make flash`.
After flashing the watch will reset and print debug info into the terminal as it's running.
**Important**: Before you disconnect the watch after flashing, exit the process printing debug info via Ctrl-C.
Otherwise the debug hardware interface of the watch will not be powered off and the battery drains much more quickly.

## Simulator

All drivers have two implementations: One for the actual watch hardware and one for a simulator.
This allows testing new firmware versions/new apps without flashing to the watch every time.
Build and start the simulator using `make simu`.

# License

Copyright 2025 ftilde

This software is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.
