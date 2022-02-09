#![no_std]
#![no_main]

extern crate panic_semihosting;

use cortex_m::{asm::delay, peripheral::DWT};
use embedded_hal::digital::v2::OutputPin;
use mbkb::physical::Physical;
use mbkb::physical::Report;
use mbkb::KeyCode;
use rtic::cyccnt::{Instant, U32Ext as _};
use stm32f1xx_hal::usb::{Peripheral, UsbBus, UsbBusType};
use stm32f1xx_hal::{gpio, prelude::*};
use usb_device::bus;
use usb_device::prelude::*;

use mbkb::physical::usb::{HIDClass, HidReport};

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
    fn on_tick(mut cx: on_tick::Context) {
        static mut KEYS: Keys<'static> = Keys {
            text: LOREM,
            last: None,
        };

        // const MESSAGE: &[KeyCode] = &[
        //     KeyCode::A,
        //     KeyCode::B,
        //     KeyCode::C,
        //     KeyCode::D,
        //     KeyCode::E,
        //     KeyCode::F,
        //     KeyCode::G,
        //     KeyCode::H,
        //     KeyCode::I,
        //     KeyCode::J,
        //     KeyCode::K,
        //     KeyCode::L,
        //     KeyCode::M,
        //     KeyCode::N,
        //     KeyCode::O,
        //     KeyCode::P,
        //     KeyCode::Q,
        //     KeyCode::R,
        //     KeyCode::S,
        //     KeyCode::T,
        //     KeyCode::U,
        //     KeyCode::V,
        //     KeyCode::W,
        //     KeyCode::X,
        //     KeyCode::Y,
        //     KeyCode::Z,
        //     KeyCode::Enter,
        // ];

        cx.schedule.on_tick(Instant::now() + PERIOD.cycles()).ok();

        let counter: &mut u8 = &mut cx.resources.counter;
        let led = &mut cx.resources.led;
        let hid = &mut cx.resources.hid;

        const P: u8 = 2;
        *counter = (*counter + 1) % P;

        if KEYS.text.is_empty() {
            *KEYS = Keys {
                text: LOREM,
                last: None,
            };
        }

        let mut report = HidReport::empty();
        if *counter < P / 2 {
            led.set_high().ok();

            //report.press(MESSAGE[*offset]);

            //MESSAGE.iter().for_each(|&kc| report.press(kc));
            KEYS.for_each(|kc| report.press(kc));
        } else {
            led.set_low().ok();

            //*offset = (*offset + 1) % MESSAGE.len();
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

struct Keys<'l> {
    text: &'l [u8],
    last: Option<u8>,
}

impl Iterator for Keys<'_> {
    type Item = KeyCode;

    fn next(&mut self) -> Option<Self::Item> {
        match self.text {
            // TODO: we probably should compare keycode, not ascii char
            &[x, ref xs @ ..] if cont(x, self.last) => {
                let (key, _shift) = KeyCode::from_ascii(x).unwrap();
                self.last = Some(key as _);
                self.text = xs;
                Some(key)
            }
            [] | [_, ..] => {
                self.last = None;
                None
            }
        }
    }
}

fn cont(x: u8, last: Option<u8>) -> bool {
    match last {
        None => true,
        Some(last) => (KeyCode::from_ascii(x).unwrap().0 as u8) > last,
    }
}
