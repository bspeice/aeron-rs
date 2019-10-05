//! Buffer that is safe to use in a multi-process/multi-thread context. Typically used for
//! handling atomic updates of memory-mapped buffers.
use std::mem::size_of;
use std::ops::Deref;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::util::{AeronError, IndexT, Result};
use std::ptr::{read_volatile, write_volatile};

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

    fn bounds_check<T>(&self, offset: IndexT) -> Result<()> {
        if offset < 0 || self.buffer.len() - (offset as usize) < size_of::<T>() {
            Err(AeronError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    #[allow(clippy::cast_ptr_alignment)]
    fn overlay<T>(&self, offset: IndexT) -> Result<&T>
    where
        T: Sized,
    {
        self.bounds_check::<T>(offset).map(|_| {
            let offset_ptr = unsafe { self.buffer.as_ptr().offset(offset as isize) };
            unsafe { &*(offset_ptr as *const T) }
        })
    }

    fn overlay_volatile<T>(&self, offset: IndexT) -> Result<T>
    where
        T: Copy
    {
        self.bounds_check::<T>(offset).map(|_| {
            let offset_ptr = unsafe { self.buffer.as_ptr().offset(offset as isize) };
            unsafe { read_volatile(offset_ptr as *const T) }
        })
    }

    fn write_volatile<T>(&mut self, offset: IndexT, val: T) -> Result<()>
    where
        T: Copy,
    {
        self.bounds_check::<T>(offset).map(|_| {
            let offset_ptr = unsafe { self.buffer.as_mut_ptr().offset(offset as isize) };
            unsafe { write_volatile(offset_ptr as *mut T, val) };
        })
    }

    /// Atomically fetch the current value at an offset, and increment by delta
    pub fn get_and_add_i64(&self, offset: IndexT, delta: i64) -> Result<i64> {
        self.overlay::<AtomicI64>(offset)
            .map(|a| a.fetch_add(delta, Ordering::SeqCst))
    }

    /// Perform a volatile read
    pub fn get_i64_volatile(&self, offset: IndexT) -> Result<i64> {
        // QUESTION: Would it be better to express this in terms of an atomic read?
        self.overlay_volatile::<i64>(offset)
    }

    /// Perform a volatile write into the buffer
    pub fn put_i64_ordered(&mut self, offset: IndexT, val: i64) -> Result<()> {
        self.write_volatile::<i64>(offset, val)
    }

    /// Compare an expected value with what is in memory, and if it matches,
    /// update to a new value. Returns `Ok(true)` if the update was successful,
    /// and `Ok(false)` if the update failed.
    pub fn compare_and_set_i64(&self, offset: IndexT, expected: i64, update: i64) -> Result<bool> {
        // QUESTION: Do I need a volatile and atomic read here?
        // Aeron C++ uses a volatile read before the atomic operation, but I think that
        // may be redundant. In addition, Rust's `read_volatile` operation returns a
        // *copied* value; running `compare_exchange` on that copy introduces a race condition
        // because we're no longer comparing a consistent address.
        self.overlay::<AtomicI64>(offset).map(|a| {
            a.compare_exchange(expected, update, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
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

    #[test]
    fn negative_offset() {
        let mut buf = [16, 0, 0, 0, 0, 0, 0, 0];
        let atomic_buf = AtomicBuffer::wrap(&mut buf);
        assert_eq!(
            atomic_buf.get_and_add_i64(-1, 0),
            Err(AeronError::OutOfBounds)
        )
    }

    #[test]
    fn put_i64() {
        let mut buf = [0u8; 8];
        let mut atomic_buf = AtomicBuffer::wrap(&mut buf);

        atomic_buf.put_i64_ordered(0, 12).unwrap();
        assert_eq!(
            atomic_buf.get_i64_volatile(0),
            Ok(12)
        )
    }

    #[test]
    fn compare_set_i64() {
        let mut buf = [0u8; 8];
        let atomic_buf = AtomicBuffer::wrap(&mut buf);

        atomic_buf.get_and_add_i64(0, 1).unwrap();

        assert_eq!(
            atomic_buf.compare_and_set_i64(0, 0, 1),
            Ok(false)
        );
        assert_eq!(
            atomic_buf.compare_and_set_i64(0, 1, 2),
            Ok(true)
        );
        assert_eq!(
            atomic_buf.get_i64_volatile(0),
            Ok(2)
        );
    }
}
