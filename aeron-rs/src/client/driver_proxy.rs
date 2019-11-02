//! Proxy object for interacting with the Media Driver. Handles operations
//! involving the command-and-control file protocol.

use crate::client::concurrent::ringbuffer::ManyToOneRingBuffer;

/// Proxy object for operations involving the Media Driver
pub struct DriverProxy<'a> {
    _to_driver: ManyToOneRingBuffer<'a>,
    _client_id: i64,
}

impl<'a> DriverProxy<'a> {}
