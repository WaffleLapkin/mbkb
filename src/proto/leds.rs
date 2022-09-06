#[derive(Copy, Clone, Debug)]
pub struct LedStates {
    pub num_lock: LedState,
    pub caps_lock: LedState,
    pub scroll_lock: LedState,
    pub compose: LedState,
    pub kana: LedState,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LedState {
    Disabled,
    Enabled,
}

impl LedState {
    pub fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub fn disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }
}
