//! Flyweight implementation for commands to terminate the driver
use crate::command::correlated_message::CorrelatedMessageDefn;
use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;
use crate::util::{IndexT, Result};
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

    /// Return the token buffer length
    pub fn token_length(&self) -> i32 {
        self.get_struct().token_length
    }

    /// Return the current token payload associated with this termination request.
    pub fn token_buffer(&self) -> &[u8] {
        // UNWRAP: Size check performed during initialization
        &self
            .bytes_at(size_of::<TerminateDriverDefn>() as IndexT)
            .unwrap()[..self.get_struct().token_length as usize]
    }

    /// Append a payload to the termination request.
    pub fn put_token_buffer(&mut self, token_buffer: &[u8]) -> Result<&mut Self> {
        let token_length = token_buffer.len() as i32;
        if token_length > 0 {
            self.buffer.put_slice(
                size_of::<TerminateDriverDefn>() as IndexT,
                &token_buffer,
                0,
                token_length,
            )?
        }
        self.get_struct_mut().token_length = token_length;
        Ok(self)
    }

    /// Get the total byte length of this termination command
    pub fn length(&self) -> IndexT {
        size_of::<Self>() as IndexT + self.get_struct().token_length
    }
}

#[cfg(test)]
mod tests {
    use crate::command::correlated_message::CorrelatedMessageDefn;
    use crate::command::flyweight::Flyweight;
    use crate::command::terminate_driver::TerminateDriverDefn;
    use crate::concurrent::AtomicBuffer;
    use crate::util::IndexT;

    use std::mem::size_of;

    #[test]
    fn terminate_command_size() {
        assert_eq!(
            size_of::<TerminateDriverDefn>(),
            size_of::<aeron_driver_sys::aeron_terminate_driver_command_stct>()
        )
    }

    #[test]
    #[should_panic]
    fn panic_on_invalid_length() {
        // QUESTION: Should this failure condition be included in the docs?
        let token_len = 1;

        // Can trigger panic if `token_length` contains a bad value during initialization
        let mut bytes = &mut [0u8; size_of::<TerminateDriverDefn>()][..];
        // `token_length` stored immediately following the correlated message, this is
        // how to calculate the offset
        let token_length_offset = size_of::<CorrelatedMessageDefn>();

        // When running inside a `should_panic` test, a failed test is one that returns at all
        let put_result = bytes.put_i32(token_length_offset as IndexT, token_len);
        if put_result.is_err() {
            return;
        }

        let flyweight = Flyweight::new::<TerminateDriverDefn>(bytes, 0);
        if flyweight.is_err() {
            return;
        }

        let flyweight = flyweight.unwrap();
        if flyweight.token_length() != token_len {
            return;
        }
        flyweight.token_buffer();
    }
}
