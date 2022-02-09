#![no_std]
#![no_main]

extern crate panic_semihosting;

use core::iter::Cycle;

use cortex_m::{asm::delay, peripheral::DWT};
use embedded_hal::digital::v2::OutputPin;
use mbkb::{
    physical::{
        usb::{HIDClass, HidReport},
        Physical, Report,
    },
    KeyCode,
};
use rtic::cyccnt::{Instant, U32Ext as _};
use stm32f1xx_hal::{
    gpio,
    prelude::*,
    usb::{Peripheral, UsbBus, UsbBusType},
};
use usb_device::{bus, prelude::*};

type LED = gpio::gpioc::PC13<gpio::Output<gpio::PushPull>>;

const PERIOD: u32 = 800_000;

#[rtic::app(device = stm32f1xx_hal::stm32, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        counter: u8,
        led: LED,

        usb_dev: UsbDevice<'static, UsbBusType>,
        hid: HIDClass<'static, UsbBusType>,
    }

    #[init(schedule = [on_tick])]
    fn init(mut cx: init::Context) -> init::LateResources {
        static mut USB_BUS: Option<bus::UsbBusAllocator<UsbBusType>> = None;

        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        let mut flash = cx.device.FLASH.constrain();
        let mut rcc = cx.device.RCC.constrain();

        let mut gpioc = cx.device.GPIOC.split(&mut rcc.apb2);
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid());

        let mut gpioa = cx.device.GPIOA.split(&mut rcc.apb2);

        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
        usb_dp.set_low().ok();
        delay(clocks.sysclk().0 / 100);

        let usb_dm = gpioa.pa11;
        let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

        let usb = Peripheral {
            usb: cx.device.USB,
            pin_dm: usb_dm,
            pin_dp: usb_dp,
        };

        *USB_BUS = Some(UsbBus::new(usb));

        let hid = HIDClass::new(USB_BUS.as_ref().unwrap());

        let usb_dev = UsbDeviceBuilder::new(USB_BUS.as_ref().unwrap(), UsbVidPid(0xc410, 0x0000))
            .manufacturer("Fake company")
            .product("not a mouse")
            .serial_number("TEST")
            .device_class(0)
            .build();

        // Wait some time so usb can connect
        cx.schedule.on_tick(cx.start + (PERIOD * 64).cycles()).ok();

        init::LateResources {
            counter: 0,
            led,

            usb_dev,
            hid,
        }
    }

    #[task(schedule = [on_tick], resources = [counter, led, hid])]
    fn on_tick(cx: on_tick::Context) {
        static mut KEYS: Option<Cycle<Keys<'static>>> = None;

        cx.schedule.on_tick(Instant::now() + PERIOD.cycles()).ok();

        let counter = &mut *cx.resources.counter;
        let led = &mut *cx.resources.led;
        let hid = &mut *cx.resources.hid;
        let keys = KEYS.get_or_insert_with(|| Keys { text: LOREM }.cycle());

        const P: u8 = 2;
        *counter = (*counter + 1) % P;

        let mut report = HidReport::empty();
        if *counter < P / 2 {
            led.set_high().ok();

            keys.next()
                .into_iter()
                .flatten()
                .for_each(|kc| report.press(kc));
        } else {
            led.set_low().ok();
        }

        hid.set_report(report);
    }

    #[task(binds=USB_HP_CAN_TX, resources = [counter, led, usb_dev, hid])]
    fn usb_tx(mut cx: usb_tx::Context) {
        usb_poll(
            &mut cx.resources.counter,
            &mut cx.resources.led,
            &mut cx.resources.usb_dev,
            &mut cx.resources.hid,
        );
    }

    #[task(binds=USB_LP_CAN_RX0, resources = [counter, led, usb_dev, hid])]
    fn usb_rx(mut cx: usb_rx::Context) {
        usb_poll(
            &mut cx.resources.counter,
            &mut cx.resources.led,
            &mut cx.resources.usb_dev,
            &mut cx.resources.hid,
        );
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::nop();
        }
    }

    extern "C" {
        fn EXTI0();
    }
};

fn usb_poll<B: bus::UsbBus>(
    _counter: &mut u8,
    _led: &mut LED,
    usb_dev: &mut UsbDevice<'static, B>,
    hid: &mut HIDClass<'static, B>,
) {
    if !usb_dev.poll(&mut [hid]) {
        return;
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
struct Keys<'l> {
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
struct KeySequence<'l> {
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
