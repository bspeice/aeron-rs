//! Flyweight implementation for commands to terminate the driver
use crate::command::correlated_message::CorrelatedMessageDefn;
use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;
use crate::util::IndexT;
use std::mem::size_of;

/// Raw command to terminate a driver. The `token_length` describes the length
/// of a buffer immediately trailing this struct definition and part of the
/// same message.
#[repr(C, packed(4))]
pub struct TerminateDriverDefn {
    pub(in crate::command) correlated_message: CorrelatedMessageDefn,
    pub(in crate::command) token_length: i32,
}

impl<A> Flyweight<A, TerminateDriverDefn>
where
    A: AtomicBuffer,
{
    /// Retrieve the client identifier of this request.
    pub fn client_id(&self) -> i64 {
        self.get_struct().correlated_message.client_id
    }

    /// Set the client identifier of this request.
    pub fn put_client_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.client_id = value;
        self
    }

    /// Retrieve the correlation identifier associated with this request. Used to
    /// associate driver responses with a specific request.
    pub fn correlation_id(&self) -> i64 {
        self.get_struct().correlated_message.correlation_id
    }

    /// Set the correlation identifier to be used with this request.
    pub fn put_correlation_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.correlation_id = value;
        self
    }

    /// Get the current length of the payload associated with this termination request.
    pub fn token_length(&self) -> i32 {
        self.get_struct().token_length
    }

    /// Set the payload length of this termination request.
    ///
    /// NOTE: While there are no safety issues, improperly setting this value can cause panics.
    /// The `token_length` value is automatically set during calls to `put_token_buffer()`,
    /// so this method is not likely to be frequently used.
    pub fn put_token_length(&mut self, value: i32) -> &mut Self {
        self.get_struct_mut().token_length = value;
        self
    }

    /// Return the current token payload associated with this termination request.
    pub fn token_buffer(&self) -> &[u8] {
        // QUESTION: Should I be slicing the buffer to `token_length`?
        // C++ doesn't do anything, so I'm going to assume not.
        &self.bytes_at(size_of::<TerminateDriverDefn>() as IndexT)
    }

    /// Append a payload to the termination request.
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

    /// Get the total byte length of this termination command
    pub fn length(&self) -> IndexT {
        size_of::<Self>() as IndexT + self.token_length()
    }
}

#[cfg(test)]
mod tests {
    use crate::command::terminate_driver::TerminateDriverDefn;
    use std::mem::size_of;

    #[test]
    fn terminate_command_size() {
        assert_eq!(
            size_of::<TerminateDriverDefn>(),
            size_of::<aeron_driver_sys::aeron_terminate_driver_command_stct>()
        )
    }
}
