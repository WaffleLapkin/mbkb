# Connect to gdb remote server
target remote :3333

file ../target/thumbv7m-none-eabi/debug/f103

# Enable demangling asm names on disassembly
set print asm-demangle on

# Enable pretty printing
set print pretty on

# Disable style sources as the default colors can be hard to read
# set style sources off

# Help gdb find rust std sources (from https://users.rust-lang.org/t/solved-how-to-step-into-std-source-code-when-debugging-in-vs-code/25319/6)
set substitute-path /rustc/c07a8b4e09f356c7468b69c50cac7fc5b5000b8a/ /home/waffle/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/

# Set a breakpiont at DefaultHandler
break DefaultHandler

# Set a breakpiont at HardFault
break HardFault

set history save on
