#![no_std]
#![no_main]

extern crate panic_semihosting;

#[rtic::app(device = stm32f1xx_hal::stm32, peripherals = true, dispatchers = [EXTI0])]
mod app {
    use cortex_m::{asm::delay, peripheral::DWT};
    use mbkb::{
        phy::{self, Layout},
        proto::{
            usb::{UsbV1, UsbV1Report},
            KeyCode, Protocol, Report,
        },
    };
    use stm32f1xx_hal::{
        gpio::{ErasedPin, Input, PullUp},
        prelude::*,
        usb::{Peripheral, UsbBus, UsbBusType},
    };
    use systick_monotonic::*;

    use usb_device::{bus, class::UsbClass, prelude::*};

    #[monotonic(binds = SysTick, default = true)]
    type Mono = Systick<100>; // 100 Hz / 10 ms granularity

    #[local]
    struct Local {
        phy_layout: phy::layouts::Array<ErasedPin<Input<PullUp>>, 4>,
    }

    #[shared]
    struct Shared {
        #[lock_free]
        usb_dev: UsbDevice<'static, UsbBusType>,
        #[lock_free]
        proto: UsbV1<'static, UsbBusType>,

        #[lock_free]
        blinky: crate::blinky::BlinkyClass<
            UsbBusType,
            stm32f1xx_hal::gpio::gpioc::PC13<
                stm32f1xx_hal::gpio::Output<stm32f1xx_hal::gpio::PushPull>,
            >,
        >,
        #[lock_free]
        webusb: usbd_webusb::WebUsb<UsbBusType>,
    }

    #[init(local = [usb_bus: Option<bus::UsbBusAllocator<UsbBusType>> = None])]
    fn init(mut cx: init::Context) -> (Shared, Local, init::Monotonics) {
        // I do not remember what this does (waffle)
        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        // Clock initialization
        let (clocks, mono) = {
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

            (clocks, mono)
        };

        // Setup usb
        let (usb_dev, proto, usb_bus) = {
            let mut gpioa = cx.device.GPIOA.split();

            // BluePill board has a pull-up resistor on the D+ line.
            // Pull the D+ pin down to send a RESET condition to the USB bus.
            let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
            usb_dp.set_low();

            // Magic delay (I don't know why I've written this) (waffle)
            delay(clocks.sysclk().0 / 100);

            let usb_dm = gpioa.pa11;
            let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

            let usb = Peripheral {
                usb: cx.device.USB,
                pin_dm: usb_dm,
                pin_dp: usb_dp,
            };

            let usb_bus = cx.local.usb_bus.insert(UsbBus::new(usb));

            let proto = UsbV1::new(usb_bus);

            let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0xc410, 0x0000))
                .manufacturer("Fake company")
                .product("not a mouse")
                .serial_number("TEST")
                .device_class(0)
                .build();

            (usb_dev, proto, &*usb_bus)
        };

        let phy_layout = {
            let mut gpiob = cx.device.GPIOB.split();
            let pins = [
                gpiob.pb12.into_pull_up_input(&mut gpiob.crh).erase(),
                gpiob.pb13.into_pull_up_input(&mut gpiob.crh).erase(),
                gpiob.pb14.into_pull_up_input(&mut gpiob.crh).erase(),
                gpiob.pb15.into_pull_up_input(&mut gpiob.crh).erase(),
            ];

            phy::layouts::Array::new(pins)
        };

        // Spawn `on_tick` task that will poll buttons.
        //
        // Wait some time so usb can connect first.
        on_tick::spawn_after(1.secs()).ok();

        let mut gpioc = cx.device.GPIOC.split();
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        led.set_high();

        let local = Local { phy_layout };
        let shared = Shared {
            usb_dev,
            proto,
            blinky: crate::blinky::BlinkyClass::new(usb_bus, led),
            webusb: usbd_webusb::WebUsb::new(
                usb_bus,
                usbd_webusb::url_scheme::HTTPS,
                "does not really matter",
            ),
        };

        (shared, local, init::Monotonics(mono))
    }

    #[task(local = [phy_layout], shared=[proto])]
    fn on_tick(cx: on_tick::Context) {
        // Repeat the same task after 16 ms
        on_tick::spawn_after(16.millis()).ok();

        let proto = &mut *cx.shared.proto;
        let phy_layout = &mut *cx.local.phy_layout;

        let mut report = UsbV1Report::empty();

        phy_layout.poll().for_each(|key| {
            let (kc, shiftness) = KeyCode::from_ascii(b"AaBb"[key.into_raw() as usize]).unwrap();
            if shiftness {
                report.press(KeyCode::LShift);
            }
            report.press(kc);
        });

        proto.set_report(report);
    }

    #[task(binds=USB_HP_CAN_TX, shared=[usb_dev, proto, blinky, webusb])]
    fn usb_tx(mut cx: usb_tx::Context) {
        usb_poll(
            &mut cx.shared.usb_dev,
            cx.shared.proto.usb_class(),
            cx.shared.blinky,
            cx.shared.webusb,
        );
    }

    #[task(binds=USB_LP_CAN_RX0, shared=[usb_dev, proto, blinky, webusb])]
    fn usb_rx(mut cx: usb_rx::Context) {
        usb_poll(
            &mut cx.shared.usb_dev,
            cx.shared.proto.usb_class(),
            cx.shared.blinky,
            cx.shared.webusb,
        );
    }

    fn usb_poll<B>(
        usb_dev: &mut UsbDevice<'_, B>,
        hid: &mut dyn UsbClass<B>,
        b: &mut dyn UsbClass<B>,
        w: &mut dyn UsbClass<B>,
    ) where
        B: bus::UsbBus,
    {
        if !usb_dev.poll(&mut [hid, b, w]) {
            return;
        }
    }
}

mod blinky {
    use core::marker::PhantomData;
    use embedded_hal::digital::v2::OutputPin;
    use usb_device::class_prelude::*;

    pub struct BlinkyClass<B: UsbBus, LED> {
        spooky: core::marker::PhantomData<B>,
        led: LED,
    }

    impl<B: UsbBus, LED: OutputPin> BlinkyClass<B, LED> {
        pub fn new(_alloc: &UsbBusAllocator<B>, led: LED) -> Self {
            Self {
                spooky: PhantomData,
                led,
            }
        }
    }

    impl<B: UsbBus, LED: OutputPin> UsbClass<B> for BlinkyClass<B, LED> {
        fn control_out(&mut self, xfer: ControlOut<B>) {
            let req = xfer.request();

            if req.request_type == control::RequestType::Vendor
                && req.recipient == control::Recipient::Device
                && req.request == 1
            {
                if req.value > 0 {
                    self.led.set_low().ok();
                } else {
                    self.led.set_high().ok();
                }
            }
        }
    }
}
