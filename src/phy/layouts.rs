use core::convert::Infallible;

use embedded_hal::digital::v2::InputPin;

use crate::phy::{KeyId, Layout};

/// Array physical layout - every key has it's own pin.
///
/// This layout is "effective" when there are no more than 4 keys. If you have
/// more than 4 keys, `Matrix` layout (to-be-implemented) uses less pins for the
/// same amount of keys. Since keyboards rarely have this few keys, this layout
/// is only useful for testing purposes.
///
/// **Note**: this layout expects **pull up** pins, i.e. low = key is pressed,
/// high = key is depressed.
pub struct Array<P, const N: usize> {
    pins: [P; N],
}

impl<P, const N: usize> Array<P, N> {
    /// Creates new array physical layout.
    ///
    /// **Note**: this expects **pull up** pins, i.e. low = key pressed, high =
    /// key is depressed.
    pub fn new(pins: [P; N]) -> Self {
        Self { pins }
    }
}

impl<P, const N: usize> Layout for Array<P, N>
where
    P: InputPin<Error = Infallible>,
{
    fn poll(&mut self, f: &mut dyn FnMut(&mut dyn Iterator<Item = KeyId>)) {
        let mut pressed = [false; N];

        self.pins
            .iter()
            .zip(&mut pressed)
            // Unwrap: Error = Infallible
            .for_each(|(pin, state)| *state = pin.is_low().unwrap());

        let mut iter = pressed
            .iter()
            .copied()
            .enumerate()
            .filter(|&(_, pressed)| pressed)
            .map(|(k, _)| KeyId::from_raw(k as u16));

        f(iter.by_ref())
    }

    fn max_key_id(&self) -> KeyId {
        KeyId::from_raw(N as _)
    }
}
