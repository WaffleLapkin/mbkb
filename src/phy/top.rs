use crate::phy::KeyId;

pub struct Repr<'a> {
    pub keys: &'a [KeyPos],
    pub centre: (f32, f32),
}

pub struct KeyPos {
    pub id: KeyId,
    pub x: f32,
    pub y: f32,
    pub rotation_rad: f32,
}
