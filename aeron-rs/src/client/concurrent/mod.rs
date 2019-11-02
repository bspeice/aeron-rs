//! Module for handling safe interactions among the multiple clients making use
//! of a single Media Driver

pub mod ringbuffer;
use std::mem::size_of;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::util::{AeronError, IndexT, Result};
use std::ptr::{read_volatile, write_volatile};

use std::ops::{DerefMut, Deref};

/// Atomic operations on slices of memory
pub trait AtomicBuffer: Deref<Target = [u8]> + DerefMut<Target = [u8]> {

    /// Check that there are at least `size` bytes of memory available
    /// beginning at some offset.
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
    ///
    /// let buffer = &mut [0u8; 8][..];
    /// assert!(buffer.bounds_check(0, 8).is_ok());
    /// assert!(buffer.bounds_check(1, 7).is_ok());
    /// assert!(buffer.bounds_check(1, 8).is_err());
    /// assert!(buffer.bounds_check(-1, 8).is_err());
    /// ```
    fn bounds_check(&self, offset: IndexT, size: IndexT) -> Result<()> {
        if offset < 0 || size < 0 || self.deref().len() as IndexT - offset < size {
            Err(AeronError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    /// Overlay a struct on a buffer.
    ///
    /// NOTE: Has the potential to cause undefined behavior if alignment is incorrect.
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
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
            T: Sized
    {
        self.bounds_check(offset, size_of::<T>() as IndexT)
            .map(|_| {
                let offset_ptr = unsafe { self.as_ptr().offset(offset as isize) };
                unsafe { &*(offset_ptr as *const T) }
            })
    }

    /// Overlay a struct on a buffer, and perform a volatile read
    ///
    /// ```rust
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
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
    /// # use aeron_rs::client::concurrent::AtomicBuffer;
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
    fn get_and_add_i64(&self, offset: IndexT, value: i64) -> Result<i64> {
        self.overlay::<AtomicI64>(offset).map(|a| a.fetch_add(value, Ordering::SeqCst))
    }
}

impl AtomicBuffer for Vec<u8> {}

impl AtomicBuffer for &mut [u8] {}
