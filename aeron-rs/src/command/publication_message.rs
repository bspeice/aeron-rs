//! Flyweight implementation for command to add a publication
use crate::command::correlated_message::CorrelatedMessageDefn;
use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;
use crate::util::{IndexT, Result};
use std::mem::size_of;

/// Control message for adding a publication
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                         Client ID                             |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                       Correlation ID                          |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                         Stream ID                             |
/// +---------------------------------------------------------------+
/// |                       Channel Length                          |
/// +---------------------------------------------------------------+
/// |                          Channel                             ...
///...                                                              |
/// +---------------------------------------------------------------+
/// ```
#[repr(C, packed(4))]
pub struct PublicationMessageDefn {
    correlated_message: CorrelatedMessageDefn,
    stream_id: i32,
    channel_length: i32,
}

// Rust has no `offset_of` macro, so we'll just compute by hand
const CHANNEL_LENGTH_OFFSET: IndexT =
    (size_of::<CorrelatedMessageDefn>() + size_of::<i32>()) as IndexT;

impl<A> Flyweight<A, PublicationMessageDefn>
where
    A: AtomicBuffer,
{
    /// Retrieve the client identifier associated with this message
    pub fn client_id(&self) -> i64 {
        self.get_struct().correlated_message.client_id
    }

    /// Set the client identifier for this message
    pub fn put_client_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.client_id = value;
        self
    }

    /// Retrieve the correlation identifier associated with this message.
    /// Will uniquely identify a command and response pair.
    pub fn correlation_id(&self) -> i64 {
        self.get_struct().correlated_message.correlation_id
    }

    /// Set the correlation identifier for this message
    pub fn put_correlation_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlated_message.correlation_id = value;
        self
    }

    /// Retrieve the stream identifier associated with this request
    pub fn stream_id(&self) -> i32 {
        self.get_struct().stream_id
    }

    /// Set the stream identifier of this request
    pub fn put_stream_id(&mut self, value: i32) -> &mut Self {
        self.get_struct_mut().stream_id = value;
        self
    }

    /// Retrieve the channel name of this request
    pub fn channel(&self) -> Result<&str> {
        self.string_get(CHANNEL_LENGTH_OFFSET)
    }

    /// Set the channel name of this request
    pub fn put_channel(&mut self, value: &str) -> Result<&mut Self> {
        self.string_put(CHANNEL_LENGTH_OFFSET, value).map(|_| self)
    }

    /// Get the total byte length of this subscription command
    pub fn length(&self) -> IndexT {
        size_of::<PublicationMessageDefn>() as IndexT + self.get_struct().channel_length
    }
}
