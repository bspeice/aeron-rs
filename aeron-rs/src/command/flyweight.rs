use crate::concurrent::AtomicBuffer;
use crate::util::{IndexT, Result};
use std::marker::PhantomData;

pub struct Flyweight<A, S>
where
    A: AtomicBuffer,
{
    pub(in crate::command) buffer: A,
    base_offset: IndexT,
    _phantom: PhantomData<S>,
}

pub struct Unchecked;

impl<A> Flyweight<A, Unchecked>
where
    A: AtomicBuffer,
{
    pub fn new<S>(buffer: A, offset: IndexT) -> Result<Flyweight<A, S>>
    where
        S: Sized
    {
        buffer.overlay::<S>(offset)?;
        Ok(Flyweight {
            buffer,
            base_offset: offset,
            _phantom: PhantomData
        })
    }
}

impl<A, S> Flyweight<A, S>
where
    A: AtomicBuffer,
    S: Sized,
{
    pub(crate) fn get_struct(&self) -> &S {
        // UNWRAP: Bounds check performed during initialization
        self.buffer.overlay::<S>(self.base_offset).unwrap()
    }

    pub(crate) fn get_struct_mut(&mut self) -> &mut S {
        // UNWRAP: Bounds check performed during initialization
        self.buffer.overlay_mut::<S>(self.base_offset).unwrap()
    }

    pub(crate) fn bytes_at(&self, offset: IndexT) -> &[u8] {
        let offset = (self.base_offset + offset) as usize;
        // FIXME: Unwrap is unjustified here.
        // C++ uses pointer arithmetic with no bounds checking, so I'm more comfortable
        // with the Rust version at least panicking. Is the idea that we're safe because
        // this is a crate-local (protected in C++) method?
        self.buffer.bounds_check(offset as IndexT, 0).unwrap();
        &self.buffer[offset..]
    }
}
