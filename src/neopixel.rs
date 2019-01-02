use core::cell::RefCell;
use cortex_m::interrupt::{self, Mutex};

pub type Generator = fn(strip_id: u8, strip_length: u8, frame_n: u32, pixel_n: u8) -> Option<BRG>;
pub type Transfer = fn(buffer: &[u8], length: u32) -> Option<BRG>;

#[derive(Clone)]
pub enum DisplayType {
    Generator(Generator),
    Static(BRG)
}

pub struct ReadWriteBuffer<'a> {
    current_read_is_1: bool,
    buffer_1: &'a mut [u8],
    buffer_2: &'a mut [u8],
    buffer_length: u16,
}

impl<'a> ReadWriteBuffer<'a> {
    pub fn new(buffer_1: &'a mut [u8], buffer_2: &'a mut [u8], buffer_length: u16) -> ReadWriteBuffer<'a> {
        Self {
            current_read_is_1: false,
            buffer_1,
            buffer_2,
            buffer_length,
        }
    }

    fn borrow(&self) -> &[u8] {
        if self.current_read_is_1 { self.buffer_1 } else { self.buffer_2 }
    }

    fn borrow_mut(&mut self) -> &mut [u8] {
        if self.current_read_is_1 { &mut self.buffer_2 } else { &mut self.buffer_1 }
    }

    pub fn swap(&mut self) { interrupt::free(|cs| self.current_read_is_1 = !self.current_read_is_1); }

    pub fn length(&self) -> u16 { self.buffer_length }
}

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

pub struct ChannelConfig<'a> {
    strip_id: u8,
    // huart: &UART_HandleTypeDef,
    buffer: ReadWriteBuffer<'a>,
    frame_n: u32,
    calculate_frame: bool,
    last_frame_calculated: bool,
    last_frame_shown: bool,
    strip_length: u8,
    brightness: u8,
    display_type: DisplayType
}

impl<'a> ChannelConfig<'a> {
    pub fn new(strip_id: u8, buffer: ReadWriteBuffer<'a>, strip_length: u8) -> ChannelConfig<'a> {
        ChannelConfig {
            strip_id: strip_id,
            buffer: buffer,
            frame_n: 0,
            calculate_frame: true,
            last_frame_calculated: false,
            last_frame_shown: false,
            strip_length: strip_length,
            brightness: 0,
            display_type: DisplayType::Static(BRG::off())
        }
    }

    const MASK: [u8; 2] = [0b11110, 0b10000];

    fn write_pixel_to_buffer(&mut self, pixel_n: usize, pixel: &BRG) {
        // Get the current buffer
        let buffer: &mut [u8] = self.buffer.borrow_mut();

        // Convert all the RGB data into a NeoPixel data stream
        // Go through all three RGB uint8_t
        let p = pixel.into_u32(self.brightness) as usize;
        for i in 0..12 {
            // Get the corresponding bits two at a time
            let d0 = (p >> i * 2) & 1;
            let d1 = (p >> (i * 2 + 1)) & 1;
            // Write the proper masks into the NeoPixel data stream
            let index: usize = pixel_n * 12 + (11 - i);
            buffer[index] = (Self::MASK[d0] << 4) | (Self::MASK[d1] >> 1);
        }
    }

    fn neopixel_poll_frame(&mut self) {
        // Only calculate a new frame if one is needed.
        // This is the case when the new frame was calculated and the old one is deprecated.
        // We know the new frame was calculated already when the read and write buffer are the same
        // as the frame calculator sets the read buffer to the write buffer when it's done
        if self.calculate_frame && !self.last_frame_calculated {
            // Iterate over all needed pixels
            for pixel_n in 0..self.strip_length {
                // Generate the pixel data
                match self.display_type.clone() {
                    DisplayType::Generator(generator) => {
                        if let Some(color) = generator(self.strip_id, self.strip_length, self.frame_n, pixel_n) {
                            self.write_pixel_to_buffer(pixel_n as usize, &color);
                        } else {
                            // If we have been signalized to have calculated the last frame, signalize this to the animation internals
                            if pixel_n == self.strip_length - 1 {
                                self.last_frame_calculated = true;
                            }
                        }
                    },
                    DisplayType::Static(color) => { self.write_pixel_to_buffer(pixel_n as usize, &color); }
                }
            }
            self.frame_n += 1;

            // Signalize that the frame was calculated and we don't need to calculate another at the moment.
            self.calculate_frame = false;	// work is done
            // Swap buffers
            self.buffer.swap();
        }
    }

    fn animation_ended(&mut self) -> bool {
        // If we have the last frame calculated flag set, we need to wait the last frame and then stop the DMA
        if self.last_frame_calculated {
            // The last frame was shown; stop the DMA
            if self.last_frame_shown {
                return true;
            }
            self.last_frame_shown = true;
        }
        return false;
    }

    pub fn start_transfer(&mut self, transfer: Transfer) {
        // Start the reocurring DMA transfer
        transfer(self.buffer.borrow(), self.strip_length as u32 * 12);
        // Remember that this frame was calculated
        // Once a new frame is needed, the reader will set this flag again
        self.calculate_frame = true;
    }

    // This callback should occur every 16.667ms to ensure 60fps
    fn start_frame(&mut self, transfer: Transfer) {
        if !self.animation_ended() {
            self.start_transfer(transfer);
        }
    }
}