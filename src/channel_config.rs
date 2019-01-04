use cortex_m::interrupt;
use generic_array::{ ArrayLength };

use crate::read_write_buffer::ReadWriteBuffer;
use crate::brg::*;
use crate::consumer::Consumer;
use crate::producer::Producer;

pub type Generator = fn(strip_id: u8, strip_length: u8, frame_n: u32, pixel_n: u8) -> Option<BRG>;
pub type Transfer = fn(buffer: &[u8], length: u32) -> Option<BRG>;

#[derive(Clone)]
pub enum DisplayType {
    Generator(Generator),
    Static(BRG)
}

pub struct ChannelConfig<N>
where
    N: ArrayLength<u8>
{
    pub(crate) strip_id: u8,
    pub(crate) buffer: ReadWriteBuffer<N>,
    producer_taken: bool,
    consumer_taken: bool,
    pub(crate) frame_n: u32,
    pub(crate) frame_requested: bool,
    pub(crate) last_frame_calculated: bool,
    pub(crate) last_frame_shown: bool,
    pub(crate) strip_length: u8,
    pub(crate) brightness: u8,
    pub(crate) display_type: DisplayType,
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
    pub fn try_swap(&mut self) {
        // This if can only ever be fulfilled if the last frame was finished with a propper swap request.
        // A swap request can only ever happen again after a frame request was issued.
        // This effectively means this if cannot be triggered again before the frame_requested is set to true.
        if self.swap_requested {
            // It is safe to swap the buffers here as we manually verified that ::try_swap() is only called when no read is in process!
            unsafe { self.buffer.swap() };
            self.swap_requested = false;
            // IMPORTANT: This has to be at the end of this if!
            self.frame_requested = true;
        }
    }
}