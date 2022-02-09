use usb_device::{class_prelude::*, Result};

use crate::{physical, physical::Report, KeyCode};

pub const USB_CLASS_HID: u8 = 0x03;

const USB_SUBCLASS_NONE: u8 = 0x00;
//const USB_SUBCLASS_BOOT: u8 = 0x01;

//const USB_INTERFACE_NONE: u8 = 0x00;
const USB_INTERFACE_KEYBOARD: u8 = 0x01;
//const USB_INTERFACE_MOUSE: u8 = 0x02;

// As defined in https://www.usb.org/sites/default/files/hid1_11.pdf p 49/59 (wtitten/real)
const DESCRIPTOR_TYPE_HID: u8 = 0x21;
const DESCRIPTOR_TYPE_REPORT: u8 = 0x22;
//const DESCRIPTOR_TYPE_PHYSICAL: u8 = 0x23;

const REQ_GET_REPORT: u8 = 0x01;
// const REQ_GET_IDLE: u8 = 0x02;
// const REQ_GET_PROTOCOL: u8 = 0x03;
// const REQ_SET_REPORT: u8 = 0x09;
// const REQ_SET_IDLE: u8 = 0x0a;
// const REQ_SET_PROTOCOL: u8 = 0x0b;

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

pub struct HidReport([u8; 13]);

impl Report for HidReport {
    fn empty() -> Self {
        Self(<_>::default())
    }

    fn press(&mut self, kc: KeyCode) {
        if kc == KeyCode::No {
            return;
        }
        // TODO: modifiers

        let idx = kc as u8 - 1;

        self.0[(idx / 8) as usize] |= 1 << (idx % 8);
    }
}

pub fn report(x: bool) -> [u8; 13] {
    let mut ret = [0; 13];
    if x {
        ret[1] |= !0; // A?
    }

    ret
}

pub struct HIDClass<'a, B: UsbBus> {
    report: HidReport,
    report_if: InterfaceNumber,
    report_ep: EndpointIn<'a, B>,
}

impl<B: UsbBus> HIDClass<'_, B> {
    /// Creates a new HIDClass with the provided UsbBus and max_packet_size in
    /// bytes. For full-speed devices, max_packet_size has to be one of 8,
    /// 16, 32 or 64.
    pub fn new(alloc: &UsbBusAllocator<B>) -> HIDClass<'_, B> {
        HIDClass {
            report: HidReport::empty(),
            report_if: alloc.interface(),
            report_ep: alloc.interrupt(16, 10),
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.report_ep.write(data).ok();
    }
}

impl<B: UsbBus> physical::Physical for HIDClass<'_, B> {
    type Report = HidReport;

    fn set_report(&mut self, report: Self::Report) {
        if self.report.0 != report.0 {
            self.write(&report.0);
        }

        self.report = report;
    }

    fn clear(&mut self) {
        self.report = HidReport::empty();
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

        if req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.report_if) as u16
        {
            match req.request {
                REQ_GET_REPORT => {
                    xfer.accept_with(&self.report.0).ok();
                }
                _ => {
                    xfer.reject().ok();
                }
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
