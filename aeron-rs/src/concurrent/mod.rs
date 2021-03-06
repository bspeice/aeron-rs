//! Module for handling safe interactions among the multiple clients making use
//! of a single Media Driver

pub mod broadcast;
pub mod ringbuffer;
use std::mem::size_of;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::util::{AeronError, IndexT, Result};
use std::ptr::{read_volatile, write_volatile};

use memmap::MmapMut;
use std::ops::{Deref, DerefMut};

fn bounds_check_slice(slice: &[u8], offset: IndexT, size: IndexT) -> Result<()> {
    if offset < 0 || size < 0 || slice.len() as IndexT - offset < size {
        Err(AeronError::OutOfBounds)
    } else {
        Ok(())
    }
}

/// Atomic operations on slices of memory
pub trait AtomicBuffer: Deref<Target = [u8]> + DerefMut<Target = [u8]> {
    /// Check that there are at least `size` bytes of memory available
    /// beginning at some offset.
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    ///
    /// let buffer = &mut [0u8; 8][..];
    /// assert!(buffer.bounds_check(0, 8).is_ok());
    /// assert!(buffer.bounds_check(1, 7).is_ok());
    /// assert!(buffer.bounds_check(1, 8).is_err());
    /// assert!(buffer.bounds_check(-1, 8).is_err());
    /// ```
    fn bounds_check(&self, offset: IndexT, size: IndexT) -> Result<()> {
        bounds_check_slice(self.deref(), offset, size)
    }

    /// Overlay a struct on a buffer.
    ///
    /// NOTE: Has the potential to cause undefined behavior if alignment is incorrect.
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// # use std::sync::atomic::{AtomicI64, Ordering};
    /// let buffer = &mut [0u8; 9][..];
    ///
    /// let my_val: &AtomicI64 = buffer.overlay::<AtomicI64>(0).unwrap();
    /// assert_eq!(my_val.load(Ordering::SeqCst), 0);
    ///
    /// my_val.store(1, Ordering::SeqCst);
    /// assert_eq!(buffer, [1, 0, 0, 0, 0, 0, 0, 0, 0]);
    ///
    /// let another_val: &AtomicI64 = buffer.overlay::<AtomicI64>(1).unwrap();
    /// assert_eq!(another_val.load(Ordering::SeqCst), 0);
    /// ```
    fn overlay<T>(&self, offset: IndexT) -> Result<&T>
    where
        T: Sized,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.as_ptr().offset(offset as isize) };
                unsafe { &*(offset_ptr as *const T) }
            })
    }

    /// Overlay a mutable value on the buffer.
    ///
    /// NOTE: Has the potential to cause undefined behavior if alignment is incorrect
    fn overlay_mut<T>(&mut self, offset: IndexT) -> Result<&mut T>
    where
        T: Sized,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.as_mut_ptr().offset(offset as isize) };
                unsafe { &mut *(offset_ptr as *mut T) }
            })
    }

    /// Overlay a struct on a buffer, and perform a volatile read
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let buffer = &mut [5, 0, 0, 0][..];
    ///
    /// let my_val: u32 = buffer.overlay_volatile::<u32>(0).unwrap();
    /// assert_eq!(my_val, 5);
    /// ```
    fn overlay_volatile<T>(&self, offset: IndexT) -> Result<T>
    where
        T: Copy,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.as_ptr().offset(offset as isize) };
                unsafe { read_volatile(offset_ptr as *const T) }
            })
    }

    /// Perform a volatile write of a value over a buffer
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buffer = &mut [0, 0, 0, 0][..];
    ///
    /// let value: u32 = 24;
    /// buffer.write_volatile(0, value).unwrap();
    /// assert_eq!(buffer, [24, 0, 0, 0]);
    /// ```
    fn write_volatile<T>(&mut self, offset: IndexT, val: T) -> Result<()>
    where
        T: Copy,
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.as_mut_ptr().offset(offset as isize) };
                unsafe { write_volatile(offset_ptr as *mut T, val) };
            })
    }

    /// Perform an atomic fetch and add of a 64-bit value
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buf = vec![0u8; 8];
    /// assert_eq!(buf.get_and_add_i64(0, 1), Ok(0));
    /// assert_eq!(buf.get_and_add_i64(0, 1), Ok(1));
    /// ```
    fn get_and_add_i64(&self, offset: IndexT, value: i64) -> Result<i64> {
        self.overlay::<AtomicI64>(offset)
            .map(|a| a.fetch_add(value, Ordering::SeqCst))
    }

    /// Perform an atomic Compare-And-Swap of a 64-bit value. Returns `Ok(true)`
    /// if the update was successful, and `Ok(false)` if the update failed.
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buf = &mut [0u8; 8][..];
    /// // Set value to 1
    /// buf.get_and_add_i64(0, 1).unwrap();
    ///
    /// // Set value to 1 if existing value is 0
    /// assert_eq!(buf.compare_and_set_i64(0, 0, 1), Ok(false));
    /// // Set value to 2 if existing value is 1
    /// assert_eq!(buf.compare_and_set_i64(0, 1, 2), Ok(true));
    /// assert_eq!(buf.get_i64_volatile(0), Ok(2));
    /// ```
    fn compare_and_set_i64(&self, offset: IndexT, expected: i64, update: i64) -> Result<bool> {
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

    /// Perform a volatile read of an `i64` value
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let buffer = vec![12u8, 0, 0, 0, 0, 0, 0, 0];
    /// assert_eq!(buffer.get_i64_volatile(0), Ok(12));
    /// ```
    fn get_i64_volatile(&self, offset: IndexT) -> Result<i64> {
        // QUESTION: Would it be better to express this in terms of an atomic read?
        self.overlay_volatile::<i64>(offset)
    }

    /// Read an `i64` value from the buffer without performing any synchronization
    fn get_i64(&self, offset: IndexT) -> Result<i64> {
        self.overlay::<i64>(offset).map(|i| *i)
    }

    /// Perform a volatile write of an `i64` value
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buffer = vec![0u8; 8];
    /// buffer.put_i64_ordered(0, 12);
    /// assert_eq!(buffer.get_i64_volatile(0), Ok(12));
    /// ```
    fn put_i64_ordered(&mut self, offset: IndexT, value: i64) -> Result<()> {
        self.write_volatile::<i64>(offset, value)
    }

    /// Write an `i64` value into the buffer without performing any synchronization
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buffer = vec![0u8; 8];
    /// buffer.put_i64(0, 12);
    /// assert_eq!(buffer.get_i64(0), Ok(12));
    /// ```
    fn put_i64(&mut self, offset: IndexT, value: i64) -> Result<()> {
        self.overlay_mut::<i64>(offset).map(|i| *i = value)
    }

    /// Write the contents of a byte slice to this buffer. Does not perform any synchronization
    fn put_slice(
        &mut self,
        index: IndexT,
        source: &[u8],
        source_index: IndexT,
        len: IndexT,
    ) -> Result<()> {
        self.bounds_check(index, len)?;
        bounds_check_slice(source, source_index, len)?;

        let index = index as usize;
        let source_index = source_index as usize;
        let len = len as usize;

        self[index..index + len].copy_from_slice(&source[source_index..source_index + len]);
        Ok(())
    }

    /// Write the contents of one buffer to another. Does not perform any synchronization
    fn put_bytes<B>(
        &mut self,
        index: IndexT,
        source: &B,
        source_index: IndexT,
        len: IndexT,
    ) -> Result<()>
    where
        B: AtomicBuffer,
    {
        self.bounds_check(index, len)?;
        source.bounds_check(source_index, len)?;

        let index = index as usize;
        let source_index = source_index as usize;
        let len = len as usize;

        self[index..index + len].copy_from_slice(&source[source_index..source_index + len]);
        Ok(())
    }

    /// Repeatedly write a value into an atomic buffer. Guaranteed to use `memset`.
    fn set_memory(&mut self, offset: IndexT, length: usize, value: u8) -> Result<()> {
        self.bounds_check(offset, length as IndexT).map(|_| unsafe {
            self.as_mut_ptr()
                .offset(offset as isize)
                .write_bytes(value, length)
        })
    }

    /// Perform a volatile read of an `i32` from the buffer
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let buffer = vec![0, 12, 0, 0, 0];
    /// assert_eq!(buffer.get_i32_volatile(1), Ok(12));
    /// ```
    fn get_i32_volatile(&self, offset: IndexT) -> Result<i32> {
        self.overlay_volatile::<i32>(offset)
    }

    /// Read an `i32` value from the buffer without performing any synchronization
    fn get_i32(&self, offset: IndexT) -> Result<i32> {
        self.overlay::<i32>(offset).map(|i| *i)
    }

    /// Perform a volatile write of an `i32` into the buffer
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut bytes = vec![0u8; 4];
    /// bytes.put_i32_ordered(0, 12);
    /// assert_eq!(bytes.get_i32_volatile(0), Ok(12));
    /// ```
    fn put_i32_ordered(&mut self, offset: IndexT, value: i32) -> Result<()> {
        self.write_volatile::<i32>(offset, value)
    }

    /// Write an `i32` value into the buffer without performing any synchronization
    ///
    /// ```rust
    /// # use aeron_rs::concurrent::AtomicBuffer;
    /// let mut buffer = vec![0u8; 5];
    /// buffer.put_i32(0, 255 + 1);
    /// assert_eq!(buffer.get_i32(1), Ok(1));
    /// ```
    fn put_i32(&mut self, offset: IndexT, value: i32) -> Result<()> {
        self.overlay_mut::<i32>(offset).map(|i| *i = value)
    }

    /// Return the total number of bytes in this buffer
    fn capacity(&self) -> IndexT {
        self.len() as IndexT
    }
}

impl AtomicBuffer for Vec<u8> {}

impl AtomicBuffer for &mut [u8] {}

impl AtomicBuffer for MmapMut {}
