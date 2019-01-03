#[derive(Clone)]
pub struct BRG {
    b: u8,
    r: u8,
    g: u8
}

impl BRG {
    pub fn new(b: u8, r: u8, g: u8) -> Self { Self { b, r, g } }

    pub fn b(mut self, b: u8) -> Self { self.b = b; self }

    pub fn r(mut self, r: u8) -> Self { self.r = r; self }

    pub fn g(mut self, g: u8) -> Self { self.g = g; self }

    pub fn off() -> Self { Self::new(0, 0, 0) }

    pub fn into_u32(&self, a: u8) -> u32 {
        ((self.b as u32 * a as u32) / 255) << 16
      | ((self.r as u32 * a as u32) / 255) << 8
      | ((self.g as u32 * a as u32) / 255) << 0
    }
}