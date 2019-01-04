use generic_array::{ ArrayLength };

use crate::channel_config::{ ChannelConfig, Transfer };

unsafe impl<'a, N> Send for Consumer<'a, N> where N: ArrayLength<u8> {}
pub struct Consumer<'a, N> where N: ArrayLength<u8> {
    pub buf: &'a mut ChannelConfig<N>,
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
           self.buf.last_frame_shown = true;
        }
        return false;
    }

    // This callback should occur every 16.667ms to ensure 60fps
    pub fn start_frame(&mut self, transfer: Transfer) {
        if !self.animation_ended() {
            self.buf.try_swap();
            transfer(&self.buf.buffer.borrow(), self.buf.strip_length as u32 * 12);
        }
    }
}