#![no_std]
#![no_main]

extern crate panic_semihosting;

use mbkb::KeyCode;

#[rtic::app(device = stm32f1xx_hal::stm32, peripherals = true, dispatchers = [EXTI0])]
mod app {
    use core::iter::Cycle;

    use cortex_m::{asm::delay, peripheral::DWT};
    use mbkb::physical::{
        usb::{HIDClass, HidReport},
        Physical, Report,
    };
    use stm32f1xx_hal::{
        prelude::*,
        usb::{Peripheral, UsbBus, UsbBusType},
    };
    use systick_monotonic::*;

    use usb_device::{bus, prelude::*};

    use crate::{Keys, LOREM};

    #[monotonic(binds = SysTick, default = true)]
    type Mono = Systick<100>; // 100 Hz / 10 ms granularity

    #[local]
    struct Resources {
        counter: u8,
        keys: Cycle<Keys<'static>>,
    }

    #[shared]
    struct Shared {
        #[lock_free]
        usb_dev: UsbDevice<'static, UsbBusType>,
        #[lock_free]
        hid: HIDClass<'static, UsbBusType>,
    }

    #[init(local = [USB_BUS: Option<bus::UsbBusAllocator<UsbBusType>> = None])]
    fn init(mut cx: init::Context) -> (Shared, Resources, init::Monotonics) {
        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        let mut flash = cx.device.FLASH.constrain();
        let rcc = cx.device.RCC.constrain();

        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid());

        // Initialize the monotonic
        let mono = Systick::new(cx.core.SYST, clocks.sysclk().0);

        let mut gpioa = cx.device.GPIOA.split();

        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
        usb_dp.set_low();
        delay(clocks.sysclk().0 / 100);

        let usb_dm = gpioa.pa11;
        let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

        let usb = Peripheral {
            usb: cx.device.USB,
            pin_dm: usb_dm,
            pin_dp: usb_dp,
        };

        let usb_bus = cx.local.USB_BUS.insert(UsbBus::new(usb));

        let hid = HIDClass::new(usb_bus);

        let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0xc410, 0x0000))
            .manufacturer("Fake company")
            .product("not a mouse")
            .serial_number("TEST")
            .device_class(0)
            .build();

        // Wait some time so usb can connect
        on_tick::spawn_after(1.secs()).ok();

        let local = Resources {
            counter: 0,
            keys: Keys { text: LOREM }.cycle(),
        };
        let shared = Shared { usb_dev, hid };

        (shared, local, init::Monotonics(mono))
    }

    #[task(local = [counter, keys], shared=[hid])]
    fn on_tick(cx: on_tick::Context) {
        on_tick::spawn_after(16.millis()).ok();

        let counter = &mut *cx.local.counter;
        let hid = &mut *cx.shared.hid;
        let keys = &mut *cx.local.keys;

        *counter = (*counter + 1) % 2;

        let mut report = HidReport::empty();

        // Send empty report every second
        if *counter == 0 {
            keys.next()
                .into_iter()
                .flatten()
                .for_each(|kc| report.press(kc));
        }

        hid.set_report(report);
    }

    #[task(binds=USB_HP_CAN_TX, shared=[usb_dev, hid])]
    fn usb_tx(mut cx: usb_tx::Context) {
        usb_poll(&mut cx.shared.usb_dev, &mut cx.shared.hid);
    }

    #[task(binds=USB_LP_CAN_RX0, shared=[usb_dev, hid])]
    fn usb_rx(mut cx: usb_rx::Context) {
        usb_poll(&mut cx.shared.usb_dev, &mut cx.shared.hid);
    }

    fn usb_poll<B: bus::UsbBus>(
        usb_dev: &mut UsbDevice<'static, B>,
        hid: &mut HIDClass<'static, B>,
    ) {
        if !usb_dev.poll(&mut [hid]) {
            return;
        }
    }
}

const LOREM: &[u8] = b"\
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis
nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu
fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in
culpa qui officia deserunt mollit anim id est laborum.\n
";

/// Converts ascii text to key presses. Note that between key sequences you need
/// to depress all keys.
#[derive(Clone)]
pub struct Keys<'l> {
    text: &'l [u8],
}

impl<'l> Iterator for Keys<'l> {
    type Item = KeySequence<'l>;

    fn next(&mut self) -> Option<Self::Item> {
        let k = |&c: &_| KeyCode::from_ascii(c).unwrap().0;
        next_group_by(&mut self.text, |a, b| k(a) < k(b)).map(|text| KeySequence { text })
    }
}

/// A sequence of keys that can surely be pressed at the same time
pub struct KeySequence<'l> {
    text: &'l [u8],
}

impl Iterator for KeySequence<'_> {
    type Item = KeyCode;

    fn next(&mut self) -> Option<Self::Item> {
        match self.text {
            &[x, ref xs @ ..] => {
                self.text = xs;
                Some(KeyCode::from_ascii(x).unwrap().0)
            }
            [] => None,
        }
    }
}

/// Takes next "group" from `slice` and returns it
///
/// "group" is defined as a slice of maximum lenght such that
/// `group.iter().all(f)`.
fn next_group_by<'l, T>(slice: &mut &'l [T], mut f: impl FnMut(&T, &T) -> bool) -> Option<&'l [T]> {
    // impl stolen from core::slice::GroupBy::next
    if slice.is_empty() {
        None
    } else {
        let mut len = 1;
        let mut iter = slice.windows(2);
        while let Some([l, r]) = iter.next() {
            if f(l, r) {
                len += 1
            } else {
                break;
            }
        }
        let (head, tail) = slice.split_at(len);
        *slice = tail;
        Some(head)
    }
}
