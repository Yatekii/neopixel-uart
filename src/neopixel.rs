use core::cell::RefCell;
use cortex_m::interrupt::{self, Mutex};
use generic_array::{ GenericArray, ArrayLength, sequence::GenericSequence };

pub type Generator = fn(strip_id: u8, strip_length: u8, frame_n: u32, pixel_n: u8) -> Option<BRG>;
pub type Transfer = fn(buffer: &[u8], length: u32) -> Option<BRG>;

#[derive(Clone)]
pub enum DisplayType {
    Generator(Generator),
    Static(BRG)
}

pub struct ReadWriteBuffer<N>
where
    N: ArrayLength<u8>
{
    current_read_is_1: bool,
    buffer_1: GenericArray<u8, N>,
    buffer_2: GenericArray<u8, N>,
    buffer_length: u16,
    needs_swap: bool,
}

impl<N> ReadWriteBuffer<N>
where
    N: ArrayLength<u8>
{
    pub fn new() -> ReadWriteBuffer<N> {
        Self {
            current_read_is_1: false,
            buffer_1: GenericArray::generate(|_| 0),
            buffer_2: GenericArray::generate(|_| 0),
            buffer_length: 0,
            needs_swap: false,
        }
    }

    fn borrow(&self) -> &[u8] {
        if self.current_read_is_1 { &self.buffer_1 } else { &self.buffer_2 }
    }

    fn borrow_mut(&mut self) -> &mut [u8] {
        if self.current_read_is_1 { &mut self.buffer_2 } else { &mut self.buffer_1 }
    }

    pub fn request_swap(&mut self) {
        self.needs_swap = true;
    }

    pub fn try_swap(&mut self) {
        if self.needs_swap {
            self.current_read_is_1 = !self.current_read_is_1;
            self.needs_swap = false;
        }
        // TODO: Do we need a CS (calls in multiple ISRs)
        //interrupt::free(|_cs| self.current_read_is_1 = !self.current_read_is_1);
    }

    pub fn length(&self) -> u16 { self.buffer_length }
}

unsafe impl<'a, N> Send for Producer<'a, N> where N: ArrayLength<u8> {}
pub struct Producer<'a, N>
where
    N: ArrayLength<u8>
{
    pub buf: &'a mut ChannelConfig<'a, N>,
}

unsafe impl<'a, N> Send for Consumer<'a, N> where N: ArrayLength<u8> {}
pub struct Consumer<'a, N>
where
    N: ArrayLength<u8>
{
    pub buf: &'a mut ChannelConfig<'a, N>,
}

pub struct Buf<N: ArrayLength<u8>> {
    buf: GenericArray<u8, N>,
    locked: bool,
}

impl<N: ArrayLength<u8>> Buf<N> {
    pub fn new() -> Buf<N> { Buf { locked: false, buf: GenericArray::generate(|_| 0) } }
    
    pub fn try_borrow_mut(&self) -> Option<&mut [u8]> {
        // TODO:
        // interrupt::free(|_cs| {
            if !self.locked {
                unsafe {
                    *(&self.locked as *const bool as *mut bool) = true;
                    Some(&mut *(self.buf.as_slice() as *const [u8] as *mut [u8]))
                }
            } else {
                None
            }
        //})
    }
    
    pub fn give_back(&self, buf: &mut [u8]) -> bool {
        if self.buf.as_slice() as *const [u8] == buf as *const [u8] {
            unsafe { *(&self.locked as *const bool as *mut bool) = false };
            true
        } else {
            false
        }
    }
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

pub struct ChannelConfig<'a, N>
where
    N: ArrayLength<u8>
{
    strip_id: u8,
    // huart: &UART_HandleTypeDef,
    buffer: ReadWriteBuffer<N>,
    producer: Option<Producer<'a, N>>,
    consumer: Option<Consumer<'a, N>>,
    frame_n: u32,
    calculate_frame: bool,
    last_frame_calculated: bool,
    last_frame_shown: bool,
    strip_length: u8,
    brightness: u8,
    display_type: DisplayType
}

impl<'a, N> ChannelConfig<'a, N>
where
    N: ArrayLength<u8>
{
    pub fn new(strip_id: u8, buffer: ReadWriteBuffer<N>, strip_length: u8) -> ChannelConfig<'a, N> {
        ChannelConfig {
            strip_id: strip_id,
            buffer: buffer,
            producer: None,
            consumer: None,
            frame_n: 0,
            calculate_frame: true,
            last_frame_calculated: false,
            last_frame_shown: false,
            strip_length: strip_length,
            brightness: 0,
            display_type: DisplayType::Static(BRG::off())
        }
    }

    pub fn take_consumer(&'a self) -> Option<Consumer<'a, N>> {
        // TODO: Make safe
        None
    }

    pub fn take_producer(&'a self) -> Option<Producer<'a, N>> {
        // TODO: Make safe
        None
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
            self.calculate_frame = false;
            // Swap buffers
            self.buffer.request_swap();
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
    pub fn start_frame(&mut self, transfer: Transfer) {
        if !self.animation_ended() {
            self.buffer.try_swap();
            self.start_transfer(transfer);
        }
    }
}

#[cfg(test)]
mod tests {
    use heapless::consts::U8;
    use crate::neopixel::Buf;
    #[test]
    fn can_borrow() {
        let b = Buf::<U8>::new();
        assert!(b.try_borrow_mut().is_some());
    }
    
    #[test]
    fn cant_borrow_twice() {
        let b = Buf::<U8>::new();
        b.try_borrow_mut().is_some();
        assert!(b.try_borrow_mut().is_none());
    }
    
    #[test]
    fn can_take_back() {
        let b = Buf::<U8>::new();
        let a = b.try_borrow_mut().unwrap();
        b.give_back(a);
        assert!(b.try_borrow_mut().is_some());
    }

    use lazy_static::lazy_static;
    lazy_static! {
        static ref buf: Buf<U8> = {
            Buf::new()
        };
    }
    
    #[test]
    fn shared_between_functions() {
        fn a() {
            assert!(buf.try_borrow_mut().is_some());
        }
        
        fn b() {
            assert!(buf.try_borrow_mut().is_none());
        }

        a();
        b();
    }
}