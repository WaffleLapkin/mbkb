use usb_device::{class_prelude::*, Result};

use crate::proto::{KeyCode, Protocol, Report};

/// Version 1 implementation of the USB keyboard protocol.
///
/// This version uses 14 byte bitset as the report, each key translates to a
/// single bit. As such, it is N-key-rollout — there isn't an upper limit on the
/// number of keys you can press.
///
/// **Note**: in order for this to work, you need to poll the usb device
/// providing a reference to the [`usb_class`].
///
/// **Note 2**: some keys are not supported (currently supported keys are in
/// ranges `[0x01; 0x67]` and `[0xE0; 0xE7]`)
///
/// [`usb_class`]: UsbV1::usb_class
pub struct UsbV1<'a, B: UsbBus> {
    inner: HIDClass<'a, B>,
}

/// [`Report`] of the [`UsbV1`] [`Protocol`].
pub struct UsbV1Report([u8; 14]);

impl<B: UsbBus> UsbV1<'_, B> {
    /// USB [protocol] implementation (first version).
    ///
    /// [protocol]: proto::Protocol
    pub fn new(alloc: &UsbBusAllocator<B>) -> UsbV1<'_, B> {
        UsbV1 {
            inner: HIDClass {
                report: UsbV1Report::empty(),
                report_if: alloc.interface(),
                report_ep: alloc.interrupt(16, 10),
            },
        }
    }

    /// Returns the usb class implementation that needs to be polled in order
    /// for this protocol to work.
    pub fn usb_class(&mut self) -> &mut dyn UsbClass<B> /* This could return HIDClass but I'm trying to make API surface smaller */
    {
        &mut self.inner
    }
}

impl Report for UsbV1Report {
    fn empty() -> Self {
        Self(<_>::default())
    }

    fn press(&mut self, kc: KeyCode) {
        if kc == KeyCode::No {
            return;
        }

        let idx = match kc as u8 {
            // move modifiers to the start
            kc @ 0xE0..=0xE7 => kc - 0xE0,
            // - `-1` ignore kc 0
            // - `+8` move after the modifiers
            kc => kc - 1 + 8,
        };

        self.0[(idx / 8) as usize] |= 1 << (idx % 8);
    }
}

impl<B: UsbBus> Protocol for UsbV1<'_, B> {
    type Report = UsbV1Report;

    fn set_report(&mut self, report: Self::Report) {
        if self.inner.report.0 != report.0 {
            self.inner.report_ep.write(&report.0).ok();
        }

        self.inner.report = report;
    }
}

struct HIDClass<'a, B: UsbBus> {
    report: UsbV1Report,
    report_if: InterfaceNumber,
    report_ep: EndpointIn<'a, B>,
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
                0x00,                   // bCountryCode
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

const USB_CLASS_HID: u8 = 0x03;

const USB_SUBCLASS_NONE: u8 = 0x00;
//const USB_SUBCLASS_BOOT: u8 = 0x01;

//const USB_INTERFACE_NONE: u8 = 0x00;
const USB_INTERFACE_KEYBOARD: u8 = 0x01;
//const USB_INTERFACE_MOUSE: u8 = 0x02;

// As defined in https://www.usb.org/sites/default/files/hid1_11.pdf p 49/59 (written/real)
const DESCRIPTOR_TYPE_HID: u8 = 0x21;
const DESCRIPTOR_TYPE_REPORT: u8 = 0x22;
//const DESCRIPTOR_TYPE_PHYSICAL: u8 = 0x23;

const REQ_GET_REPORT: u8 = 0x01;
// const REQ_GET_IDLE: u8 = 0x02;
// const REQ_GET_PROTOCOL: u8 = 0x03;
// const REQ_SET_REPORT: u8 = 0x09;
// const REQ_SET_IDLE: u8 = 0x0a;
// const REQ_SET_PROTOCOL: u8 = 0x0b;

// This describes a keyboard report layout.
//
// 14 bytes / 112 bits.
// - bits 0..8 describe modifier keys (0xE0..=0xE7)
// - bits 8..110 describe all other keys (0x01..=0x67)
// - bits 110..112 are padding
//
// Note that modifiers must go "before" "normal" keys as we want modifiers
// affect keys pressed in the same report.
//
// FIXME: 0x01 aka ErrorRollOver can probably be ignored?
const REPORT_DESCR: &[u8] = &[
    0x05, 0x01, // USAGE_PAGE (Generic Desktop)
    0x09, 0x06, // USAGE (Keyboard)
    0xa1, 0x01, // COLLECTION (Application)
    0x05, 0x07, //   USAGE_PAGE (Keyboard/Keypad)
    0x75, 0x01, //   Report Size (1)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    //
    0x19, 0xE0, //   Usage minimum (0xE0, Left Control)
    0x29, 0xE7, //   Usage maximum (0xE7, Right Gui)
    0x95, 0x08, //   Report Count (0x08, 8)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    //
    0x19, 0x01, //   Usage minimum (0x01, Keyboard ErrorRollOver)
    0x29, 0x67, //   Usage maximum (0x67, Keypad =)
    0x95, 0x66, //   Report Count (0x66, 102)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    //
    0x95, 0x02, //   Report Count (0x02, 2)
    0x81, 0x03, //   Input (Constant, Variable, Absolute)
    0xc0, //       END_COLLECTION
];
