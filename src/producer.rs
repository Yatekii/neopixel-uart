use generic_array::{ ArrayLength };

use crate::channel_config::{ ChannelConfig, DisplayType };
use crate::brg::*;

unsafe impl<'a, N> Send for Producer<'a, N> where N: ArrayLength<u8> {}
pub struct Producer<'a, N> where N: ArrayLength<u8> {
    pub buf: &'a mut ChannelConfig<N>,
}

impl<'a, N> Producer<'a, N>
where
    N: ArrayLength<u8>
{
    const MASK: [u8; 2] = [0b11110, 0b10000];

    fn write_pixel_to_buffer(&mut self, pixel_n: usize, pixel: &BRG) {
        // Get the current buffer
        let buffer: &mut [u8] = &mut self.buf.buffer.borrow_mut();

        // Convert all the RGB data into a NeoPixel data stream
        // Go through all three RGB uint8_t
        let p = pixel.into_u32(self.buf.brightness) as usize;
        for i in 0..12 {
            // Get the corresponding bits two at a time
            let d0 = (p >> i * 2) & 1;
            let d1 = (p >> (i * 2 + 1)) & 1;
            // Write the proper masks into the NeoPixel data stream
            let index: usize = pixel_n * 12 + (11 - i);
            buffer[index] = (Self::MASK[d0] << 4) | (Self::MASK[d1] >> 1);
        }
    }

    pub fn neopixel_poll_frame(&mut self) {
        // Only calculate a new frame if one is needed.
        // This if can only ever be fulfilled if a new frame was requested which only happens after the buffer has been swapped an a transfer to UART has been started.
        if self.buf.frame_requested && !self.buf.last_frame_calculated {
            // Iterate over all needed pixels
            for pixel_n in 0..self.buf.strip_length {
                // Generate the pixel data
                match self.buf.display_type.clone() {
                    DisplayType::Generator(generator) => {
                        if let Some(color) = generator(self.buf.strip_id, self.buf.strip_length, self.buf.frame_n, pixel_n) {
                            self.write_pixel_to_buffer(pixel_n as usize, &color);
                        } else {
                            // If we have been signalized to have calculated the last frame, signalize this to the animation internals
                            if pixel_n == self.buf.strip_length - 1 {
                                self.buf.last_frame_calculated = true;
                            }
                        }
                    },
                    DisplayType::Static(color) => { self.write_pixel_to_buffer(pixel_n as usize, &color); }
                }
            }
            self.buf.frame_n += 1;
            // Signalize that the frame was calculated and we don't need to calculate another at the moment.
            self.buf.frame_requested = false;
            // Swap buffers
            self.buf.request_swap();
        }
    }
}