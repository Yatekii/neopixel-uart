use core::cell::RefCell;
use cortex_m::interrupt::{self, Mutex};
use generic_array::{ GenericArray, ArrayLength, sequence::GenericSequence };

use crate::brg::*;

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
        }
    }

    // Require an &Consumer to ensure that only a Consumer (which has to be unique for each buffer!) can ever get a mutable reference.
    fn borrow(&self, _consumer: &Consumer<N>) -> &[u8] {
        if self.current_read_is_1 { &self.buffer_1 } else { &self.buffer_2 }
    }

    fn borrow_mut(&mut self) -> &mut [u8] {
        if self.current_read_is_1 { &mut self.buffer_2 } else { &mut self.buffer_1 }
    }

    // Require a ReadWriteBuffer to be present, such that we cannot call this from too many wrong places!
    // The user of this method has to ensure that no read on the buffer is in process!
    pub unsafe fn swap(&mut self, _chc: &ChannelConfig<N>) {
        self.current_read_is_1 = !self.current_read_is_1;
    }

    pub fn length(&self) -> u16 { self.buffer_length }
}

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
        let buffer: &mut [u8] = self.buf.buffer.borrow_mut();

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

    fn neopixel_poll_frame(&mut self) {
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

unsafe impl<'a, N> Send for Consumer<'a, N> where N: ArrayLength<u8> {}
pub struct Consumer<'a, N> where N: ArrayLength<u8> {
    pub buf: &'a ChannelConfig<N>,
}

impl<'a, N> Consumer<'a, N>
where
    N: ArrayLength<u8>
{
    fn animation_ended(&mut self) -> bool {
        // If we have the last frame calculated flag set, we need to wait the last frame and then stop the DMA
        if self.buf.last_frame_calculated {
            // The last frame was shown; stop the DMA
            if self.buf.last_frame_shown {
                return true;
            }
            // This write here can be considered safe if this value only ever is set here!
            // The worst that can happen like this is, that one frame more is shown.
            unsafe { (&mut *(self.buf as *const ChannelConfig<N> as *mut ChannelConfig<N>)).last_frame_shown = true; }
        }
        return false;
    }

    pub fn start_transfer(&mut self, transfer: Transfer) {
        // Start the reocurring DMA transfer
        transfer(self.buf.buffer.borrow(self), self.buf.strip_length as u32 * 12);
    }

    // This callback should occur every 16.667ms to ensure 60fps
    pub fn start_frame(&mut self, transfer: Transfer) {
        if !self.animation_ended() {
            self.buf.try_swap(self);
            self.start_transfer(transfer);
        }
    }
}

pub struct ChannelConfig<N>
where
    N: ArrayLength<u8>
{
    strip_id: u8,
    // huart: &UART_HandleTypeDef,
    buffer: ReadWriteBuffer<N>,
    producer_taken: bool,
    consumer_taken: bool,
    frame_n: u32,
    frame_requested: bool,
    last_frame_calculated: bool,
    last_frame_shown: bool,
    strip_length: u8,
    brightness: u8,
    display_type: DisplayType,
    swap_requested: bool,
}

impl<N> ChannelConfig<N>
where
    N: ArrayLength<u8>
{
    pub fn new(strip_id: u8, buffer: ReadWriteBuffer<N>, strip_length: u8) -> ChannelConfig<N> {
        ChannelConfig {
            strip_id: strip_id,
            buffer: buffer,
            producer_taken: false,
            consumer_taken: false,
            frame_n: 0,
            frame_requested: true,
            last_frame_calculated: false,
            last_frame_shown: false,
            strip_length: strip_length,
            brightness: 0,
            display_type: DisplayType::Static(BRG::off()),
            swap_requested: false,
        }
    }

    // We can only take one consumer ever since we require this method to happen in a CriticalSection!
    pub fn take_consumer(&self, _cs: &interrupt::CriticalSection) -> Option<Consumer<N>> {
        if self.consumer_taken {
            None
        } else {
            let s = unsafe { (&mut *(self as *const ChannelConfig<N> as *mut ChannelConfig<N>)) };
            s.consumer_taken = true;
            Some(Consumer { buf: s })
        }
    }

    // We can only take one producer ever since we require this method to happen in a CriticalSection!
    pub fn take_producer(&self, _cs: &interrupt::CriticalSection) -> Option<Producer<N>> {
        if self.producer_taken {
            None
        } else {
            let s = unsafe { (&mut *(self as *const ChannelConfig<N> as *mut ChannelConfig<N>)) };
            s.producer_taken = true;
            Some(Producer { buf: s })
        }
    }

    pub fn request_swap(&mut self) {
        self.swap_requested = true;
    }

    // Require a consumer (which is unique to the ChannelConfig) to be present so only a consumer can call this!
    pub fn try_swap(&self, _consumer: &Consumer<N>) {
        // This if can only ever be fulfilled if the last frame was finished with a propper swap request.
        // A swap request can only ever happen again after a frame request was issued.
        // This effectively means this if cannot be triggered again before the frame_requested is set to true.
        if self.swap_requested {
            // It is safe to swap the buffers here as we manually verified that ::try_swap() is only called when no read is in process!
            let s = unsafe {
                let s = &mut *(self as *const ChannelConfig<N> as *mut ChannelConfig<N>);
                s.buffer.swap(self);
                s
            };
            s.swap_requested = false;
            // IMPORTANT: This has to be at the end of this if!
            s.frame_requested = true;
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