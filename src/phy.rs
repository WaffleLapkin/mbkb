/// Prebuilt [`Layout`] implementations.
pub mod layouts;

/// Things related to the **top**ology.
///
/// **Very very WIP**.
pub mod top;

/// Physical layout of a keyboard.
pub trait Layout {
    /// Returns iterator of currently pressed keys.
    ///
    /// Note that [`KeyId`]s should be stable across calls to [`poll`],
    /// executions and even non-breaking library changes. i.e. users should be
    /// able to assign meaning to [`KeyId`]s and then save it.
    ///
    /// It is preferred to choose the smallest possible [`KeyId`]s. For example:
    /// if a keyboard has 4 keys, then [`poll`] should return [`KeyId`]s in
    /// range `[KeyId(0); KeyId(4)]` and [`max_key_id`] should return
    /// `KeyId(4)`.
    ///
    /// [`poll`]: Layout::poll
    /// [`max_key_id`]: Layout::max_key_id
    fn poll(&mut self) -> &mut dyn Iterator<Item = KeyId>;

    /// Maximum [`KeyId`] that can be returned from this [`poll`].
    ///
    /// [`poll`]: Layout::poll
    fn max_key_id(&self) -> KeyId;

    fn topological_repr(&self) -> Option<top::Repr<'_>> {
        None
    }
}

/// Identifier of a physical key (switch, button, etc).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyId(u16);

impl KeyId {
    /// Creates a [`KeyId`] from the raw representation, a number.
    ///
    /// See [`Layout::poll`] documentation on how inner values should be chosen.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Converts `KeyId` back to the raw representation.
    pub const fn into_raw(self) -> u16 {
        self.0
    }
}
