#![no_std]

mod brg;
mod neopixel;
pub use crate::neopixel::*;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
