# Connect to gdb remote server
target remote :3333

# Load will flash the code
load

# Enable demangling asm names on disassembly
set print asm-demangle on

# Enable pretty printing
set print pretty on

# Disable style sources as the default colors can be hard to read
# set style sources off

# Help gdb find rust std sources (from https://users.rust-lang.org/t/solved-how-to-step-into-std-source-code-when-debugging-in-vs-code/25319/6)
set substitute-path /rustc/240ff4c4a0d0936c9eeb783fa9ff5c0507a6ffb4/ /home/waffle/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/

# Initialize monitoring so iprintln! macro output
# is sent from the itm port to itm.txt
monitor tpiu config internal itm.txt uart off 8000000

# Turn on the itm port
monitor itm port 0 on

# Set a breakpoint at main, aka entry
break main

# Set a breakpiont at DefaultHandler
break DefaultHandler

# Set a breakpiont at HardFault
break HardFault

# Continue running until we hit the main breakpoint
continue

# Step from the trampoline code in entry into main
step


set history save on
