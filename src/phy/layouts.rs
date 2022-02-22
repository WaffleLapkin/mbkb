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
    iter: ArrayIter<N>,
}

impl<P, const N: usize> Array<P, N> {
    /// Creates new array physical layout.
    ///
    /// **Note**: this expects **pull up** pins, i.e. low = key pressed, high =
    /// key is depressed.
    pub fn new(pins: [P; N]) -> Self {
        Self {
            pins,
            iter: ArrayIter {
                states: [false; N],
                position: 0,
            },
        }
    }
}

impl<P, const N: usize> Layout for Array<P, N>
where
    P: InputPin<Error = Infallible>,
{
    fn poll(&mut self) -> &mut dyn Iterator<Item = KeyId> {
        self.iter.position = 0;
        self.iter
            .states
            .iter_mut()
            .zip(&self.pins)
            // Unwrap: Error = Infallible
            .for_each(|(state, pin)| *state = pin.is_low().unwrap());

        &mut self.iter
    }

    fn max_key_id(&self) -> KeyId {
        KeyId::from_raw(N as _)
    }
}

struct ArrayIter<const N: usize> {
    // FIXME: should use bit array probably
    states: [bool; N],
    position: usize,
}

impl<const N: usize> Iterator for ArrayIter<N> {
    type Item = KeyId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= N {
            return None;
        }

        loop {
            let p = self.position;
            self.position += 1;

            if *self.states.get(p)? {
                break Some(KeyId::from_raw(p as _));
            }
        }
    }
}
