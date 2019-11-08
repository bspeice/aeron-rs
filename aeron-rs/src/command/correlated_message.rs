//! Header struct for commands that use an identifier to associate the media driver response.
use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;

/// Basic definition for messages that include a client and correlation identifier to associate
/// commands and responses
#[repr(C, packed(4))]
pub struct CorrelatedMessageDefn {
    pub(in crate::command) client_id: i64,
    pub(in crate::command) correlation_id: i64,
}

impl<A> Flyweight<A, CorrelatedMessageDefn>
where
    A: AtomicBuffer,
{
    /// Retrieve the client identifier associated with this message
    pub fn client_id(&self) -> i64 {
        self.get_struct().client_id
    }

    /// Set the client identifier for this message
    pub fn put_client_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().client_id = value;
        self
    }

    /// Retrieve the correlation identifier associated with this message.
    /// Will uniquely identify a command and response pair.
    pub fn correlation_id(&self) -> i64 {
        self.get_struct().correlation_id
    }

    /// Set the correlation identifier for this message
    pub fn put_correlation_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlation_id = value;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::command::correlated_message::CorrelatedMessageDefn;
    use std::mem::size_of;

    #[test]
    fn correlated_message_size() {
        assert_eq!(
            size_of::<CorrelatedMessageDefn>(),
            size_of::<aeron_driver_sys::aeron_correlated_command_stct>()
        )
    }
}
