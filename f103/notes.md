Just some notes so I do not forget stuff

## Description

`f103` is just a cargo project for testing the code on real hardware (I haven't figured how to test anything on an unreal hardware...)

## Setup

- STM32F103C8 aka bluepill
- 4 buttons connected to pins B12, B13, B14 and B15 (and ground, obv)

![Said setup, a bluepill dev board, pins B12 through B15 are connected via yellow wires to a breadboard and via it to simple buttons, ground is connected via a red wire to a - power-line which itself connects via red wires to buttons. Bluepill is also connected to an stlink and a red micro usb cable.](./setup.jpg)

## Flashing firmware (?)

- Connect stlink in an stlinky way
- Go to `f103` dir (important!)
  - Otherwise `f103/.cargo/config.toml` won't be used and everything'll be on fire
- Run `c embed`

## Debugging

There are two common practices for debug, `println!` and debuggers.
In this particular case it seems like they conflict with each other... for reasons? idk.

When doing `println!`-style debugging, use `rprintln!` from `rtt-target`.

When using debuggers you'll need to
- Run `c embed with_gdb` (in `f103` dir)
  - This allows `gdb` to connect, but
  - Disables rtt, you won't be able to use `rptintln!` and won't see panic messages :(
- Run `arm-none-eabi-gdb -q -x ../openocd.gdb` (in `f103` dir)
