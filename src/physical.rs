use crate::KeyCode;

pub mod usb;

pub trait Physical {
    type Report;

    fn set_report(&mut self, report: Self::Report);
    fn clear(&mut self);
}

pub trait Report {
    fn empty() -> Self;

    fn press(&mut self, kc: KeyCode);
}
