mod kc;
mod leds;
pub mod usb;

pub use kc::KeyCode;
pub use leds::{LedState, LedStates};

/// A protocol that sends information about pressed keys to the host (computer).
pub trait Protocol {
    /*
     * This is *heavily* moduled after USB.
     * This is probably not a bad thing, considering that USB is the only
     * protocol I currently plan to implement, but maybe this can be an issue if
     * implementing other protocols idk :shrug:
     */

    /// Report type that hold information about currently pressed keys.
    type Report: Report;

    /// Set the report that should be reported to the host.
    fn set_report(&mut self, report: Self::Report);

    /// Set empty report.
    fn clear(&mut self) {
        self.set_report(Self::Report::empty());
    }

    /// Returns current led states.
    fn leds(&self) -> LedStates;
}

/// Report type that hold information about currently pressed keys.
pub trait Report {
    /// Creates an empty report, i.e. a report with no pressed keys.
    fn empty() -> Self;

    /// Add a key press to this report
    fn press(&mut self, kc: KeyCode);
}
