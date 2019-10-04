//! Buffer that is safe to use in a multi-process/multi-thread context. Typically used for
//! handling atomic updates of memory-mapped buffers.
use std::mem::size_of;
use std::ops::Deref;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::util::{AeronError, IndexT, Result};

/// Wrapper for atomic operations around an underlying byte buffer
pub struct AtomicBuffer<'a> {
    buffer: &'a mut [u8],
}

impl<'a> Deref for AtomicBuffer<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buffer
    }
}

impl<'a> AtomicBuffer<'a> {
    /// Create an `AtomicBuffer` as a view on an underlying byte slice
    pub fn wrap(buffer: &'a mut [u8]) -> Self {
        AtomicBuffer { buffer }
    }

    fn bounds_check(&self, offset: IndexT, size: usize) -> Result<()> {
        if self.buffer.len() - (offset as usize) < size {
            Err(AeronError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    /// Atomically fetch the current value at an offset, and increment by delta
    #[allow(clippy::cast_ptr_alignment)]
    pub fn get_and_add_i64(&self, offset: IndexT, delta: i64) -> Result<i64> {
        self.bounds_check(offset, size_of::<AtomicI64>()).map(|_| {
            let a: &AtomicI64 =
                unsafe { &*(self.buffer.as_ptr().offset(offset as isize) as *const AtomicI64) };
            println!("AtomicI64: {:p}", a);
            a.fetch_add(delta, Ordering::SeqCst)
        })
    }
}

#[cfg(test)]
mod tests {
    use memmap::MmapOptions;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::client::concurrent::atomic_buffer::AtomicBuffer;
    use crate::util::AeronError;

    #[test]
    fn mmap_to_atomic() {
        let mut mmap = MmapOptions::new()
            .len(24)
            .map_anon()
            .expect("Unable to map anonymous memory");
        AtomicBuffer::wrap(&mut mmap);
    }

    #[test]
    fn primitive_atomic_equivalent() {
        let value: u64 = 24;

        let val_ptr = &value as *const u64;
        let a_ptr = val_ptr as *const AtomicU64;
        let a: &AtomicU64 = unsafe { &*a_ptr };

        assert_eq!(value, (*a).load(Ordering::SeqCst));
    }

    #[test]
    fn atomic_i64_increment() {
        let mut buf = [16, 0, 0, 0, 0, 0, 0, 0];

        let atomic_buf = AtomicBuffer::wrap(&mut buf[..]);
        assert_eq!(atomic_buf.get_and_add_i64(0, 1), Ok(16));
        assert_eq!(atomic_buf.get_and_add_i64(0, 0), Ok(17));
    }

    #[test]
    fn atomic_i64_increment_offset() {
        let mut buf = [0, 16, 0, 0, 0, 0, 0, 0, 0];

        let atomic_buf = AtomicBuffer::wrap(&mut buf[..]);
        assert_eq!(atomic_buf.get_and_add_i64(1, 1), Ok(16));
        assert_eq!(atomic_buf.get_and_add_i64(1, 0), Ok(17));
    }

    #[test]
    fn out_of_bounds() {
        let mut buf = [16, 0, 0, 0, 0, 0, 0];

        let atomic_buf = AtomicBuffer::wrap(&mut buf);
        assert_eq!(
            atomic_buf.get_and_add_i64(0, 0),
            Err(AeronError::OutOfBounds)
        );
    }

    #[test]
    fn out_of_bounds_offset() {
        let mut buf = [16, 0, 0, 0, 0, 0, 0, 0];

        let atomic_buf = AtomicBuffer::wrap(&mut buf);
        assert_eq!(
            atomic_buf.get_and_add_i64(1, 0),
            Err(AeronError::OutOfBounds)
        );
    }
}
