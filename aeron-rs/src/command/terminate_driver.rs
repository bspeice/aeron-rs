use crate::command::correlated_message::CorrelatedMessageDefn;
use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;
use crate::util::IndexT;
use std::mem::size_of;

pub struct TerminateDriverDefn {
    pub(crate) correlated_message: CorrelatedMessageDefn,
    pub(crate) token_length: i32,
}

impl<A> Flyweight<A, TerminateDriverDefn>
where
    A: AtomicBuffer,
{
    pub fn client_id(&self) -> i64 {
        self.get_struct().correlated_message.client_id
    }

    pub fn put_client_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.client_id = value;
        self
    }

    pub fn correlation_id(&self) -> i64 {
        self.get_struct().correlated_message.correlation_id
    }

    pub fn put_correlation_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.correlation_id = value;
        self
    }

    pub fn token_length(&self) -> i32 {
        self.get_struct().token_length
    }

    pub fn put_token_length(&mut self, value: i32) -> &mut Self {
        self.get_struct_mut().token_length = value;
        self
    }

    pub fn token_buffer(&self) -> &[u8] {
        // QUESTION: Should I be slicing the buffer to `token_length`?
        // C++ doesn't do anything, so I'm going to assume not.
        &self.bytes_at(size_of::<TerminateDriverDefn>() as IndexT)
    }

    pub fn put_token_buffer(&mut self, token_buffer: &[u8]) -> &mut Self {
        let token_length = token_buffer.len() as i32;
        self.get_struct_mut().token_length = token_length;

        if token_length > 0 {
            // FIXME: Unwrap is unjustified here
            // Currently just assume that people are going to be nice about the token buffer
            // and not oversize it.
            self.buffer
                .put_slice(
                    size_of::<TerminateDriverDefn>() as IndexT,
                    &token_buffer,
                    0,
                    token_length,
                )
                .unwrap()
        }
        self
    }

    pub fn length(&self) -> IndexT {
        size_of::<Self>() as IndexT + self.token_length()
    }
}
