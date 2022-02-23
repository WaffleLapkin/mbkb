#![no_std]

/// Things related to the **phy**sical layout of a keyboard (where keys located,
/// how to read their state, etc).
///
/// Note that this module only identifies keys (via [`KeyId`]) and does not
/// assign any meaning to them.
// FIXME: mention what does assign meaning to keys (when something will)
pub mod phy;

/// Things related to the **proto**calls that communicate with the host
/// (computer) to tell it which keys are pressed.
pub mod proto;
