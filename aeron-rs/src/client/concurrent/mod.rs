//! Module for handling safe interactions among the multiple clients making use
//! of a single Media Driver

pub mod ringbuffer;
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

    fn bounds_check(&self, offset: IndexT, size: IndexT) -> Result<()> {
        if offset < 0 || size < 0 || self.buffer.len() as IndexT - offset < size {
            Err(AeronError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    /// Overlay a struct on a buffer.
    ///
    /// NOTE: Has the potential to cause undefined behavior if alignment is incorrect.
    pub fn overlay<T>(&self, offset: IndexT) -> Result<&T>
        where
            T: Sized,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.buffer.as_ptr().offset(offset as isize) };
                unsafe { &*(offset_ptr as *const T) }
            })
    }

    fn overlay_volatile<T>(&self, offset: IndexT) -> Result<T>
        where
            T: Copy,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.buffer.as_ptr().offset(offset as isize) };
                unsafe { read_volatile(offset_ptr as *const T) }
            })
    }

    fn write_volatile<T>(&mut self, offset: IndexT, val: T) -> Result<()>
        where
            T: Copy,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.buffer.as_mut_ptr().offset(offset as isize) };
                unsafe { write_volatile(offset_ptr as *mut T, val) };
            })
    }

    /// Atomically fetch the current value at an offset, and increment by delta
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// # use aeron_rs::util::AeronError;
    /// let mut bytes = [0u8; 9];
    /// let mut buffer = AtomicBuffer::wrap(&mut bytes);
    ///
    /// // Simple case modifies only the first byte
    /// assert_eq!(buffer.get_and_add_i64(0, 1), Ok(0));
    /// assert_eq!(buffer.get_and_add_i64(0, 0), Ok(1));
    ///
    /// // Using an offset modifies the second byte
    /// assert_eq!(buffer.get_and_add_i64(1, 1), Ok(0));
    /// assert_eq!(buffer.get_and_add_i64(1, 0), Ok(1));
    ///
    /// // An offset of 2 means buffer size must be 10 to contain an `i64`
    /// assert_eq!(buffer.get_and_add_i64(2, 0), Err(AeronError::OutOfBounds));
    /// ```
    pub fn get_and_add_i64(&self, offset: IndexT, delta: i64) -> Result<i64> {
        self.overlay::<AtomicI64>(offset)
            .map(|a| a.fetch_add(delta, Ordering::SeqCst))
    }

    /// Perform a volatile read
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut bytes = [12, 0, 0, 0, 0, 0, 0, 0];
    /// let buffer = AtomicBuffer::wrap(&mut bytes);
    ///
    /// assert_eq!(buffer.get_i64_volatile(0), Ok(12));
    /// ```
    pub fn get_i64_volatile(&self, offset: IndexT) -> Result<i64> {
        // QUESTION: Would it be better to express this in terms of an atomic read?
        self.overlay_volatile::<i64>(offset)
    }

    /// Get the current value at an offset without using any synchronization operations
    pub fn get_i64(&self, offset: IndexT) -> Result<i64> {
        self.overlay::<i64>(offset).map(|i| *i)
    }

    /// Perform a volatile read
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut bytes = [12, 0, 0, 0];
    /// let buffer = AtomicBuffer::wrap(&mut bytes);
    ///
    /// assert_eq!(buffer.get_i32_volatile(0), Ok(12));
    /// ```
    pub fn get_i32_volatile(&self, offset: IndexT) -> Result<i32> {
        self.overlay_volatile::<i32>(offset)
    }

    /// Get the current value at an offset without using any synchronization operations
    pub fn get_i32(&self, offset: IndexT) -> Result<i32> {
        self.overlay::<i32>(offset).map(|i| *i)
    }

    /// Perform a volatile write of an `i64` into the buffer
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut bytes = [0u8; 8];
    /// let mut buffer = AtomicBuffer::wrap(&mut bytes);
    ///
    /// buffer.put_i64_ordered(0, 12);
    /// assert_eq!(buffer.get_i64_volatile(0), Ok(12));
    /// ```
    pub fn put_i64_ordered(&mut self, offset: IndexT, val: i64) -> Result<()> {
        // QUESTION: Would it be better to have callers use `write_volatile` directly
        self.write_volatile::<i64>(offset, val)
    }

    /// Perform a volatile write of an `i32` into the buffer
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut bytes = [0u8; 4];
    /// let mut buffer = AtomicBuffer::wrap(&mut bytes);
    ///
    /// buffer.put_i32_ordered(0, 12);
    /// assert_eq!(buffer.get_i32_volatile(0), Ok(12));
    /// ```
    pub fn put_i32_ordered(&mut self, offset: IndexT, val: i32) -> Result<()> {
        // QUESTION: Would it be better to have callers use `write_volatile` directly
        self.write_volatile::<i32>(offset, val)
    }

    /// Write the contents of one buffer to another. Does not perform any synchronization.
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut source_bytes = [1u8, 2, 3, 4];
    /// let source = AtomicBuffer::wrap(&mut source_bytes);
    ///
    /// let mut dest_bytes = [0, 0, 0, 0];
    /// let mut dest = AtomicBuffer::wrap(&mut dest_bytes);
    ///
    /// dest.put_bytes(1, &source, 1, 3);
    /// drop(dest);
    /// assert_eq!(dest_bytes, [0u8, 2, 3, 4]);
    /// ```
    pub fn put_bytes(
        &mut self,
        index: IndexT,
        source: &AtomicBuffer,
        source_index: IndexT,
        len: IndexT,
    ) -> Result<()> {
        self.bounds_check(index, len)?;
        source.bounds_check(source_index, len)?;

        let index = index as usize;
        let source_index = source_index as usize;
        let len = len as usize;
        self.buffer[index..index + len].copy_from_slice(&source[source_index..source_index + len]);
        Ok(())
    }

    /// Compare an expected value with what is in memory, and if it matches,
    /// update to a new value. Returns `Ok(true)` if the update was successful,
    /// and `Ok(false)` if the update failed.
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    /// let mut buf = [0u8; 8];
    /// let atomic_buf = AtomicBuffer::wrap(&mut buf);
    /// // Set value to 1
    /// atomic_buf.get_and_add_i64(0, 1).unwrap();
    ///
    /// // Set value to 1 if existing value is 0
    /// assert_eq!(atomic_buf.compare_and_set_i64(0, 0, 1), Ok(false));
    /// // Set value to 2 if existing value is 1
    /// assert_eq!(atomic_buf.compare_and_set_i64(0, 1, 2), Ok(true));
    /// assert_eq!(atomic_buf.get_i64_volatile(0), Ok(2));
    /// ```
    pub fn compare_and_set_i64(&self, offset: IndexT, expected: i64, update: i64) -> Result<bool> {
        // QUESTION: Should I use a volatile read here as well?
        // Aeron C++ uses a volatile read before the atomic operation, but I think that
        // may be redundant. In addition, Rust's `read_volatile` operation returns a
        // *copied* value; running `compare_exchange` on that copy introduces a race condition
        // because we're no longer comparing a consistent address.
        self.overlay::<AtomicI64>(offset).map(|a| {
            a.compare_exchange(expected, update, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        })
    }

    /// Repeatedly write a value into an atomic buffer. Guaranteed to use `memset`.
    pub fn set_memory(&mut self, offset: IndexT, length: usize, value: u8) -> Result<()> {
        self.bounds_check(offset, length as IndexT).map(|_| unsafe {
            self.buffer
                .as_mut_ptr()
                .offset(offset as isize)
                .write_bytes(value, length)
        })
    }
}

#[cfg(test)]
mod tests {
    use memmap::MmapOptions;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::client::concurrent::AtomicBuffer;
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
    fn negative_offset() {
        let mut buf = [16, 0, 0, 0, 0, 0, 0, 0];
        let atomic_buf = AtomicBuffer::wrap(&mut buf);
        assert_eq!(
            atomic_buf.get_and_add_i64(-1, 0),
            Err(AeronError::OutOfBounds)
        )
    }
}
