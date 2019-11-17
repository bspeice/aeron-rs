//! Flyweight pattern implementation for messages to and from the media driver.
use crate::concurrent::AtomicBuffer;
use crate::util::{IndexT, Result};
use std::marker::PhantomData;

/// Flyweight holder object. Wrapper around an underlying `AtomicBuffer` and
/// offset within that buffer that all future operations are relative to.
pub struct Flyweight<A, S>
where
    A: AtomicBuffer,
{
    pub(in crate::command) buffer: A,
    base_offset: IndexT,
    _phantom: PhantomData<S>,
}

/// Marker struct for an uninitialized `Flyweight` object
// We can't put this `new` method in the fully generic implementation because
// Rust gets confused as to what type `S` should be.
pub struct Unchecked;

impl<A> Flyweight<A, Unchecked>
where
    A: AtomicBuffer,
{
    /// Create a new flyweight object. Performs a bounds check on initialization
    /// to ensure there is space available for `S`.
    #[allow(clippy::new_ret_no_self)]
    pub fn new<S>(buffer: A, offset: IndexT) -> Result<Flyweight<A, S>>
    where
        S: Sized,
    {
        buffer.overlay::<S>(offset)?;
        Ok(Flyweight {
            buffer,
            base_offset: offset,
            _phantom: PhantomData,
        })
    }
}

impl<A, S> Flyweight<A, S>
where
    A: AtomicBuffer,
    S: Sized,
{
    pub(in crate::command) fn get_struct(&self) -> &S {
        // UNWRAP: Bounds check performed during initialization
        self.buffer.overlay::<S>(self.base_offset).unwrap()
    }

    pub(in crate::command) fn get_struct_mut(&mut self) -> &mut S {
        // UNWRAP: Bounds check performed during initialization
        self.buffer.overlay_mut::<S>(self.base_offset).unwrap()
    }

    pub(in crate::command) fn bytes_at(&self, offset: IndexT) -> Result<&[u8]> {
        let offset = (self.base_offset + offset) as usize;
        self.buffer.bounds_check(offset as IndexT, 0)?;
        Ok(&self.buffer[offset..])
    }

    pub(in crate::command) fn string_get(&self, offset: IndexT) -> Result<&str> {
        self.buffer
            .get_string((self.base_offset + offset) as IndexT)
    }

    pub(in crate::command) fn string_put(&mut self, offset: IndexT, value: &str) -> Result<i32> {
        self.buffer
            .put_string((self.base_offset + offset) as IndexT, value)
    }
}
