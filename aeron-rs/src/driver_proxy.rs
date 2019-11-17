//! High level API for issuing commands to the Media Driver
use crate::command::flyweight::Flyweight;
use crate::command::terminate_driver::TerminateDriverDefn;
use crate::concurrent::ringbuffer::ManyToOneRingBuffer;
use crate::concurrent::AtomicBuffer;
use crate::control_protocol::ClientCommand;
use crate::util::{AeronError, IndexT, Result};
use std::mem::size_of;
use crate::command::subscription_message::SubscriptionMessageDefn;
use std::ops::Sub;

/// High-level interface for issuing commands to a media driver
pub struct DriverProxy<A>
where
    A: AtomicBuffer,
{
    to_driver: ManyToOneRingBuffer<A>,
    client_id: i64,
}

const COMMAND_BUFFER_SIZE: usize = 512;

impl<A> DriverProxy<A>
where
    A: AtomicBuffer,
{
    /// Initialize a new driver proxy from a command-and-control "to driver" buffer
    pub fn new(to_driver: ManyToOneRingBuffer<A>) -> Self {
        let client_id = to_driver.next_correlation_id();
        DriverProxy {
            to_driver,
            client_id,
        }
    }

    /// Retrieve the timestamp of the most recent driver heartbeat. Values are
    /// milliseconds past 1 Jan 1970, UTC.
    pub fn time_of_last_driver_keepalive(&self) -> i64 {
        self.to_driver.consumer_heartbeat_time()
    }

    /// Get the unique identifier associated with this proxy.
    pub fn client_id(&self) -> i64 {
        self.client_id
    }

    /// Request the media driver subscribe to a new channel and stream.
    pub fn add_subscription(&mut self, channel: &str, stream_id: i32) -> Result<i64> {
        let correlation_id = self.to_driver.next_correlation_id();
        if channel.len() > (COMMAND_BUFFER_SIZE - size_of::<SubscriptionMessageDefn>()) {
            return Err(AeronError::InsufficientCapacity)
        }

        self.write_command_to_driver(|buffer: &mut [u8], length: &mut IndexT| {
            // UNWRAP: `SubscriptionMessageDefn` guaranteed to be smaller than `COMMAND_BUFFER_SIZE`
            let mut subscription_message = Flyweight::new::<SubscriptionMessageDefn>(buffer, 0).unwrap();

            subscription_message.put_client_id(self.client_id)
                .put_registration_correlation_id(-1)
                .put_correlation_id(correlation_id)
                .put_stream_id(stream_id);
            // UNWRAP: Bounds check performed prior to attempting the write
            subscription_message.put_channel(channel).unwrap();

            *length = subscription_message.length();
            ClientCommand::AddSubscription
        });

        Ok(correlation_id)
    }

    /// Request termination of the media driver. Optionally supply a payload on the request
    /// that the driver can use to decide whether or not to honor the request.
    pub fn terminate_driver(&mut self, token_buffer: Option<&[u8]>) -> Result<()> {
        let client_id = self.client_id;
        if token_buffer.is_some()
            && token_buffer.unwrap().len()
                > (COMMAND_BUFFER_SIZE - size_of::<TerminateDriverDefn>())
        {
            return Err(AeronError::InsufficientCapacity);
        }
        self.write_command_to_driver(|buffer: &mut [u8], length: &mut IndexT| {
            // UNWRAP: `TerminateDriverDefn` guaranteed to be smaller than `COMMAND_BUFFER_SIZE`
            let mut request = Flyweight::new::<TerminateDriverDefn>(buffer, 0).unwrap();

            request.put_client_id(client_id).put_correlation_id(-1);
            // UNWRAP: Bounds check performed prior to attempting the write
            token_buffer.map(|b| request.put_token_buffer(b).unwrap());
            *length = request.length();

            ClientCommand::TerminateDriver
        })
    }

    fn write_command_to_driver<F>(&mut self, filler: F) -> Result<()>
    where
        F: FnOnce(&mut [u8], &mut IndexT) -> ClientCommand,
    {
        // QUESTION: Can Rust align structs on stack?
        // C++ does some fancy shenanigans I assume help the CPU cache?
        let mut buffer = &mut [0u8; COMMAND_BUFFER_SIZE][..];
        let mut length = buffer.len() as IndexT;
        let msg_type_id = filler(&mut buffer, &mut length);

        if !self
            .to_driver
            .write(msg_type_id as i32, &buffer, 0, length)?
        {
            Err(AeronError::IllegalState)
        } else {
            Ok(())
        }
    }
}
