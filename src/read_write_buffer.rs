use generic_array::{ GenericArray, ArrayLength, sequence::GenericSequence };
use typenum::Unsigned;
// use core::ops::Drop;
// use core::ops::Deref;
// use core::ops::DerefMut;

// pub struct Grant<'a> {
//     pub(crate) buffer: &'a [u8],
//     pub(crate) borrowed: &'a mut bool
// }

// impl<'a> Deref for Grant<'a> {
//     type Target = [u8];

//     fn deref(&self) -> &[u8] {
//         &self.buffer
//     }
// }

// impl<'a> Drop for Grant<'a> {
//     fn drop(&mut self) {
//         *self.borrowed = false;
//     }
// }

// pub struct GrantMut<'a> {
//     buffer: &'a mut [u8],
//     borrowed: &'a mut bool
// }

// impl<'a> Deref for GrantMut<'a> {
//     type Target = [u8];

//     fn deref(&self) -> &[u8] {
//         &self.buffer
//     }
// }

// impl<'a> DerefMut for GrantMut<'a> {

//     fn deref_mut(&mut self) -> &mut [u8] {
//         self.buffer
//     }
// }

// impl<'a> Drop for GrantMut<'a> {
//     fn drop(&mut self) {
//         *self.borrowed = false;
//     }
// }

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
            buffer_length: <N as Unsigned>::to_u16(),
        }
    }

    pub(crate) fn borrow(& mut self) -> &[u8] {
        if self.current_read_is_1 {
            &self.buffer_1
        } else {
            &self.buffer_2
        }
    }

    pub(crate) fn borrow_mut<'a>(&'a mut self) -> &mut [u8] {
        if self.current_read_is_1 {
            &mut self.buffer_2
        } else {
            &mut self.buffer_1
        }
    }

    // The user of this method has to ensure that no read on the buffer is in process!
    pub unsafe fn swap(&mut self) {
        self.current_read_is_1 = !self.current_read_is_1;
    }

    pub fn length(&self) -> u16 { self.buffer_length }
}

#[cfg(test)]
mod tests {
    use typenum::U8;
    use crate::read_write_buffer::ReadWriteBuffer;

    const TEST: u8 = 42;

    #[test]
    fn can_borrow() {
        let mut b = ReadWriteBuffer::<U8>::new();
        assert_eq!(b.borrow()[0], 0);
    }
    
    #[test]
    fn can_borrow_mut() {
        let mut b = ReadWriteBuffer::<U8>::new();
        let borrow = b.borrow_mut();
        borrow[0] = TEST;
        assert_eq!(borrow[0], TEST);
    }

    #[test]
    fn can_swap() {
        let mut b = ReadWriteBuffer::<U8>::new();
        let borrow = b.borrow_mut();
        borrow[0] = TEST;
        unsafe { b.swap() };
        let borrow = b.borrow();
        assert_eq!(borrow[0], TEST);
    }

    #[test]
    #[should_panic]
    fn write_to_bad_memory() {
        let mut b = ReadWriteBuffer::<U8>::new();
        b.borrow_mut()[8] = TEST;
    }

    #[test]
    fn correct_buffer_size() {
        let b = ReadWriteBuffer::<U8>::new();
        assert_eq!(b.length(), 8);
    }

    #[test]
    fn correct_slice_len() {
        let mut b = ReadWriteBuffer::<U8>::new();
        assert_eq!(b.borrow().len(), 8);
    }

    use lazy_static::lazy_static;
    lazy_static! {
        static ref buf: ReadWriteBuffer<U8> = {
            ReadWriteBuffer::new()
        };
    }
}