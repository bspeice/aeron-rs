use crate::command::flyweight::Flyweight;
use crate::command::terminate_driver::TerminateDriverDefn;
use crate::concurrent::ringbuffer::ManyToOneRingBuffer;
use crate::concurrent::AtomicBuffer;
use crate::control_protocol::ClientCommand;
use crate::util::{AeronError, IndexT, Result};

pub struct DriverProxy<A>
where
    A: AtomicBuffer,
{
    to_driver: ManyToOneRingBuffer<A>,
    client_id: i64,
}

impl<A> DriverProxy<A>
where
    A: AtomicBuffer,
{
    pub fn new(to_driver: ManyToOneRingBuffer<A>) -> Self {
        let client_id = to_driver.next_correlation_id();
        DriverProxy {
            to_driver,
            client_id,
        }
    }

    pub fn time_of_last_driver_keepalive(&self) -> Result<i64> {
        self.to_driver.consumer_heartbeat_time()
    }

    pub fn client_id(&self) -> i64 {
        self.client_id
    }

    pub fn terminate_driver(&mut self, _token_buffer: Option<&[u8]>) -> Result<()> {
        let _client_id = self.client_id;
        self.write_command_to_driver(|buffer: &mut [u8], _length: &mut IndexT| {
            // UNWRAP: Buffer from `write_command` guaranteed to be long enough for `TerminateDriverDefn`
            let _request = Flyweight::new::<TerminateDriverDefn>(buffer, 0).unwrap();

            // FIXME: Uncommenting this causes termination to not succeed
            /*
            request.put_client_id(client_id).put_correlation_id(-1);
            token_buffer.map(|b| request.put_token_buffer(b));
            *length = request.token_length();
            */

            ClientCommand::TerminateDriver
        })
    }

    fn write_command_to_driver<F>(&mut self, filler: F) -> Result<()>
    where
        F: FnOnce(&mut [u8], &mut IndexT) -> ClientCommand,
    {
        // QUESTION: Can Rust align structs on stack?
        // C++ does some fancy shenanigans I assume help the CPU cache?
        let mut buffer = &mut [0u8; 512][..];
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
