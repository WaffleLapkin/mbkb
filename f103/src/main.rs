#![no_std]
#![no_main]

extern crate panic_semihosting;

use cortex_m::{asm::delay, peripheral::DWT};
use embedded_hal::digital::v2::OutputPin;
use rtic::cyccnt::{Instant, U32Ext as _};
use stm32f1xx_hal::usb::{Peripheral, UsbBus, UsbBusType};
use stm32f1xx_hal::{gpio, prelude::*};
use usb_device::bus;
use usb_device::prelude::*;

#[allow(unused)]
pub mod hid {
    use usb_device::class_prelude::*;
    use usb_device::Result;

    pub const USB_CLASS_HID: u8 = 0x03;

    const USB_SUBCLASS_NONE: u8 = 0x00;
    const USB_SUBCLASS_BOOT: u8 = 0x01;

    const USB_INTERFACE_NONE: u8 = 0x00;
    const USB_INTERFACE_KEYBOARD: u8 = 0x01;
    //const USB_INTERFACE_MOUSE: u8 = 0x02;

    // As defined in https://www.usb.org/sites/default/files/hid1_11.pdf p 49/59 (wtitten/real)
    const DESCRIPTOR_TYPE_HID: u8 = 0x21;
    const DESCRIPTOR_TYPE_REPORT: u8 = 0x22;
    const DESCRIPTOR_TYPE_PHYSICAL: u8 = 0x23;

    const REQ_GET_REPORT: u8 = 0x01;
    const REQ_GET_IDLE: u8 = 0x02;
    const REQ_GET_PROTOCOL: u8 = 0x03;
    const REQ_SET_REPORT: u8 = 0x09;
    const REQ_SET_IDLE: u8 = 0x0a;
    const REQ_SET_PROTOCOL: u8 = 0x0b;

    const REPORT_DESCR: &[u8] = &[
        0x05, 0x01, // USAGE_PAGE (Generic Desktop)
        0x09, 0x06, // USAGE (Keyboard)
        0xa1, 0x01, // COLLECTION (Application)
        0x05, 0x07, //   USAGE_PAGE (Keyboard/Keypad)
        0x19, 0x01, //   Usage minumum (0x01, Keyboard ErrorRollOver)
        0x29, 0x67, //   Usage maximum (0x67, Keypad =)
        0x15, 0x00, //   Logical Minimum (0)
        0x25, 0x01, //   Logical Maximum (1)
        0x75, 0x01, //   Report Size (1)
        0x95, 0x66, //   Report Count (0x66, 102)
        0x81, 0x02, //   Input (Data, Variabl, Absolute)
        0x95, 0x02, //   Report Count (0x02, 2)
        0x81, 0x03, //   Input (Constant, Variable, Absolute)
        0xc0, //       END_COLLECTION
    ];

    pub fn report(x: bool) -> [u8; 13] {
        let mut ret = [0; 13];
        if x {
            ret[1] |= !0; // A?
        }

        ret
    }

    pub struct HIDClass<'a, B: UsbBus> {
        report_if: InterfaceNumber,
        report_ep: EndpointIn<'a, B>,
    }

    impl<B: UsbBus> HIDClass<'_, B> {
        /// Creates a new HIDClass with the provided UsbBus and max_packet_size in bytes. For
        /// full-speed devices, max_packet_size has to be one of 8, 16, 32 or 64.
        pub fn new(alloc: &UsbBusAllocator<B>) -> HIDClass<'_, B> {
            HIDClass {
                report_if: alloc.interface(),
                report_ep: alloc.interrupt(16, 10),
            }
        }

        pub fn write(&mut self, data: &[u8]) {
            self.report_ep.write(data).ok();
        }
    }

    impl<B: UsbBus> UsbClass<B> for HIDClass<'_, B> {
        fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
            writer.interface(
                self.report_if,
                USB_CLASS_HID,
                USB_SUBCLASS_NONE, // TODO: may be 1 to support boot mode? https://www.usb.org/sites/default/files/hid1_11.pdf (p18)
                USB_INTERFACE_KEYBOARD,
            )?;

            let descr_len: u16 = REPORT_DESCR.len() as u16;

            // https://www.usb.org/sites/default/files/hid1_11.pdf p 22/32
            writer.write(
                DESCRIPTOR_TYPE_HID,
                &[
                    0x11,                   // bcdHID
                    0x01,                   // bcdHID (1.11)
                    0x00,                   // bContryCode
                    0x01,                   // bNumDescriptors (1)
                    DESCRIPTOR_TYPE_REPORT, // bDescriptorType (report)
                    descr_len as u8,        // wDescriptorLength
                    (descr_len >> 8) as u8, // wDescriptorLength
                ],
            )?;

            writer.endpoint(&self.report_ep)?;

            Ok(())
        }

        fn control_in(&mut self, xfer: ControlIn<B>) {
            let req = xfer.request();

            if req.request_type == control::RequestType::Standard {
                match (req.recipient, req.request) {
                    (control::Recipient::Interface, control::Request::GET_DESCRIPTOR) => {
                        let (dtype, _index) = req.descriptor_type_index();
                        if dtype == DESCRIPTOR_TYPE_HID {
                            // HID descriptor
                            cortex_m::asm::bkpt();
                            let descr_len: u16 = REPORT_DESCR.len() as u16;

                            // HID descriptor (s 6.2.1)
                            let descr = &[
                                0x09,                   // length
                                DESCRIPTOR_TYPE_HID,    // descriptor type
                                0x01,                   // bcdHID
                                0x01,                   // bcdHID
                                0x00,                   // bCountryCode
                                0x01,                   // bNumDescriptors
                                DESCRIPTOR_TYPE_REPORT, // bDescriptorType
                                descr_len as u8,        // wDescriptorLength
                                (descr_len >> 8) as u8, // wDescriptorLength
                            ];

                            xfer.accept_with(descr).ok();
                            return;
                        } else if dtype == DESCRIPTOR_TYPE_REPORT {
                            // Report descriptor
                            xfer.accept_with(REPORT_DESCR).ok();
                            return;
                        }
                    }
                    _ => {
                        return;
                    }
                };
            }

            if !(req.request_type == control::RequestType::Class
                && req.recipient == control::Recipient::Interface
                && req.index == u8::from(self.report_if) as u16)
            {
                return;
            }

            match req.request {
                REQ_GET_REPORT => {
                    // USB host requests for report
                    // I'm not sure what should we do here, so just send empty report
                    xfer.accept_with(&report(false)).ok();
                }
                _ => {
                    xfer.reject().ok();
                }
            }
        }

        fn control_out(&mut self, xfer: ControlOut<B>) {
            let req = xfer.request();

            if !(req.request_type == control::RequestType::Class
                && req.recipient == control::Recipient::Interface
                && req.index == u8::from(self.report_if) as u16)
            {
                return;
            }

            xfer.reject().ok();
        }
    }
}

use hid::HIDClass;

type LED = gpio::gpioc::PC13<gpio::Output<gpio::PushPull>>;

const PERIOD: u32 = 8_000_000;

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
            .product("mouse")
            .serial_number("TEST")
            .device_class(0)
            .build();

        cx.schedule.on_tick(cx.start + PERIOD.cycles()).ok();

        init::LateResources {
            counter: 0,
            led,

            usb_dev,
            hid,
        }
    }

    #[task(schedule = [on_tick], resources = [counter, led, hid])]
    fn on_tick(mut cx: on_tick::Context) {
        cx.schedule.on_tick(Instant::now() + PERIOD.cycles()).ok();

        let counter: &mut u8 = &mut cx.resources.counter;
        let led = &mut cx.resources.led;
        let hid = &mut cx.resources.hid;

        const P: u8 = 2;
        *counter = (*counter + 1) % P;

        // move mouse cursor horizontally (x-axis) while blinking LED
        if *counter < P / 2 {
            led.set_high().ok();
            hid.write(&hid::report(true));
        } else {
            led.set_low().ok();
            hid.write(&hid::report(false));
        }
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
