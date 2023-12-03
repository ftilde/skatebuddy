#!/bin/sh

# The following
# - initializes board specific stuff: init
# - Sets the CTRL-AP (dpreg 4) register using
#   - the old value, but
#   - clearing CSYSPWRUPREQ (28th) and CDBGPWRUPREQ (30th) bits
# - exits openocd: exit
openocd -f interface/cmsis-dap.cfg -f target/nrf52.cfg --command 'init; nrf52.dap dpreg 4 [expr {[nrf52.dap dpreg 4] & ~((1 << 28) | (1 << 30))}]; exit'

# This is important, because the debug interface draws a significant portion of
# current (1mA or so). Without this it is only disabled on power cycle which
# (on the bangle.js 2) only happens when the battery is completely drained.
