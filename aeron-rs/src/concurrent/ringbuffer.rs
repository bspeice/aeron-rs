//! Ring buffer wrapper for communicating with the Media Driver
use crate::concurrent::AtomicBuffer;
use crate::util::bit::align;
use crate::util::{bit, AeronError, IndexT, Result};
use std::ops::{Deref, DerefMut};

/// Description of the ring buffer schema
pub mod buffer_descriptor {
    use crate::util::bit::{is_power_of_two, CACHE_LINE_LENGTH};
    use crate::util::AeronError::IllegalArgument;
    use crate::util::{IndexT, Result};

    // QUESTION: Why are these offsets so large when we only ever use i64 types?

    /// Offset in the ring buffer metadata to the end of the most recent record.
    pub const TAIL_POSITION_OFFSET: IndexT = (CACHE_LINE_LENGTH * 2) as IndexT;

    /// QUESTION: Why the distinction between HEAD_CACHE and HEAD?
    pub const HEAD_CACHE_POSITION_OFFSET: IndexT = (CACHE_LINE_LENGTH * 4) as IndexT;

    /// Offset in the ring buffer metadata to index of the next record to read.
    pub const HEAD_POSITION_OFFSET: IndexT = (CACHE_LINE_LENGTH * 6) as IndexT;

    /// Offset of the correlation id counter, as measured in bytes past
    /// the start of the ring buffer metadata trailer.
    pub const CORRELATION_COUNTER_OFFSET: IndexT = (CACHE_LINE_LENGTH * 8) as IndexT;

    /// Offset within the ring buffer trailer to the consumer heartbeat timestamp
    pub const CONSUMER_HEARTBEAT_OFFSET: IndexT = (CACHE_LINE_LENGTH * 10) as IndexT;

    /// Total size of the ring buffer metadata trailer.
    pub const TRAILER_LENGTH: IndexT = (CACHE_LINE_LENGTH * 12) as IndexT;

    /// Verify the capacity of a buffer is legal for use as a ring buffer.
    /// Returns the actual capacity excluding ring buffer metadata.
    pub fn check_capacity(capacity: IndexT) -> Result<()> {
        if is_power_of_two(capacity) {
            Ok(())
        } else {
            Err(IllegalArgument)
        }
    }
}

/// Ring buffer message header. Made up of fields for message length, message type,
/// and then the encoded message.
///
/// Writing the record length signals the message recording is complete, and all
/// associated ring buffer metadata has been properly updated.
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |R|                       Record Length                         |
/// +-+-------------------------------------------------------------+
/// |                              Type                             |
/// +---------------------------------------------------------------+
/// |                       Encoded Message                        ...
///...                                                              |
/// +---------------------------------------------------------------+
/// ```
// QUESTION: What is the `R` bit in the diagram above?
pub mod record_descriptor {
    use std::mem::size_of;

    use crate::util::Result;
    use crate::util::{AeronError, IndexT};

    /// Size of the ring buffer record header.
    pub const HEADER_LENGTH: IndexT = size_of::<i32>() as IndexT * 2;

    /// Alignment size of records written to the buffer
    pub const ALIGNMENT: IndexT = HEADER_LENGTH;

    /// Message type indicating to the media driver that space has been reserved,
    /// and is not yet ready for processing.
    pub const PADDING_MSG_TYPE_ID: i32 = -1;

    pub(super) fn make_header(length: i32, msg_type_id: i32) -> i64 {
        // QUESTION: Instead of masking, can't we just cast and return u32/u64?
        // Smells like Java.
        ((i64::from(msg_type_id) & 0xFFFF_FFFF) << 32) | (i64::from(length) & 0xFFFF_FFFF)
    }

    pub(super) fn check_msg_type_id(msg_type_id: i32) -> Result<()> {
        if msg_type_id < 1 {
            Err(AeronError::IllegalArgument)
        } else {
            Ok(())
        }
    }

    pub(super) fn encoded_msg_offset(record_offset: IndexT) -> IndexT {
        record_offset + HEADER_LENGTH
    }

    /// Return the position of the record length field given a record's starting position
    pub fn length_offset(record_offset: IndexT) -> IndexT {
        record_offset
    }

    /// Return the position of the record message type field given a record's starting position
    pub fn type_offset(record_offset: IndexT) -> IndexT {
        record_offset + size_of::<i32>() as IndexT
    }

    pub(super) fn record_length(header: i64) -> i32 {
        header as i32
    }

    pub(super) fn message_type_id(header: i64) -> i32 {
        (header >> 32) as i32
    }
}

const INSUFFICIENT_CAPACITY: IndexT = -2;

/// Multi-producer, single-consumer ring buffer implementation.
pub struct ManyToOneRingBuffer<A>
where
    A: AtomicBuffer,
{
    buffer: A,
    capacity: IndexT,
    max_msg_length: IndexT,
    tail_position_index: IndexT,
    head_cache_position_index: IndexT,
    head_position_index: IndexT,
    correlation_id_counter_index: IndexT,
    consumer_heartbeat_index: IndexT,
}

impl<A> ManyToOneRingBuffer<A>
where
    A: AtomicBuffer,
{
    /// Create a many-to-one ring buffer from an underlying atomic buffer.
    pub fn new(buffer: A) -> Result<Self> {
        let capacity = buffer.capacity() - buffer_descriptor::TRAILER_LENGTH;
        buffer_descriptor::check_capacity(capacity)?;
        Ok(ManyToOneRingBuffer {
            buffer,
            capacity,
            max_msg_length: capacity / 8,
            tail_position_index: capacity + buffer_descriptor::TAIL_POSITION_OFFSET,
            head_cache_position_index: capacity + buffer_descriptor::HEAD_CACHE_POSITION_OFFSET,
            head_position_index: capacity + buffer_descriptor::HEAD_POSITION_OFFSET,
            correlation_id_counter_index: capacity + buffer_descriptor::CORRELATION_COUNTER_OFFSET,
            consumer_heartbeat_index: capacity + buffer_descriptor::CONSUMER_HEARTBEAT_OFFSET,
        })
    }

    /// Atomically retrieve the next correlation identifier. Used as a unique identifier for
    /// interactions with the Media Driver
    pub fn next_correlation_id(&self) -> i64 {
        // UNWRAP: Known-valid offset calculated during initialization
        self.buffer
            .get_and_add_i64(self.correlation_id_counter_index, 1)
            .unwrap()
    }

    /// Return the total number of bytes in this buffer
    pub fn capacity(&self) -> IndexT {
        self.capacity
    }

    /// Write a message into the ring buffer
    pub fn write<B>(
        &mut self,
        msg_type_id: i32,
        source: &B,
        source_index: IndexT,
        length: IndexT,
    ) -> Result<bool>
    where
        B: AtomicBuffer,
    {
        record_descriptor::check_msg_type_id(msg_type_id)?;
        self.check_msg_length(length)?;

        let record_len = length + record_descriptor::HEADER_LENGTH;
        let required = bit::align(record_len as usize, record_descriptor::ALIGNMENT as usize);
        let record_index = self.claim_capacity(required as IndexT)?;

        if record_index == INSUFFICIENT_CAPACITY {
            return Ok(false);
        }

        // UNWRAP: `claim_capacity` performed bounds checking
        self.buffer
            .put_i64_ordered(
                record_index,
                record_descriptor::make_header(-length, msg_type_id),
            )
            .unwrap();
        // UNWRAP: `claim_capacity` performed bounds checking
        self.buffer
            .put_bytes(
                record_descriptor::encoded_msg_offset(record_index),
                source,
                source_index,
                length,
            )
            .unwrap();
        // UNWRAP: `claim_capacity` performed bounds checking
        self.buffer
            .put_i32_ordered(record_descriptor::length_offset(record_index), record_len)
            .unwrap();

        Ok(true)
    }

    /// Read messages from the ring buffer and dispatch to `handler`, up to `message_count_limit`.
    /// The handler is given the message type identifier and message body as arguments.
    ///
    /// NOTE: The C++ API will stop reading and clean up if an exception is thrown in the handler
    /// function; by contrast, the Rust API makes no attempt to catch panics and currently
    /// has no way of stopping reading once started.
    pub fn read_n<F>(&mut self, mut handler: F, message_count_limit: usize) -> Result<usize>
    where
        F: FnMut(i32, &[u8]) -> (),
    {
        let head = self.buffer.get_i64(self.head_position_index)?;
        let head_index = (head & i64::from(self.capacity - 1)) as i32;
        let contiguous_block_length = self.capacity - head_index;
        let mut messages_read = 0;
        let mut bytes_read: i32 = 0;

        let result: Result<()> = (|| {
            while bytes_read < contiguous_block_length && messages_read < message_count_limit {
                let record_index = head_index + bytes_read;
                let header = self.buffer.get_i64_volatile(record_index)?;
                let record_length = record_descriptor::record_length(header);

                if record_length <= 0 {
                    break;
                }

                bytes_read += align(
                    record_length as usize,
                    record_descriptor::ALIGNMENT as usize,
                ) as i32;

                let msg_type_id = record_descriptor::message_type_id(header);
                if msg_type_id == record_descriptor::PADDING_MSG_TYPE_ID {
                    // QUESTION: Is this a spinlock on a writer finishing?
                    continue;
                }

                messages_read += 1;
                let msg_start = record_descriptor::encoded_msg_offset(record_index) as usize;
                let msg_end =
                    msg_start + (record_length - record_descriptor::HEADER_LENGTH) as usize;
                handler(msg_type_id, &self.buffer[msg_start..msg_end]);
            }
            Ok(())
        })();

        // C++ has much better semantics for handling cleanup like this; however, because
        // it would require us to capture a mutable reference to self, it's not feasible
        // in Rust (since the main operation also needs mutable access to self).
        let mut cleanup = || {
            if bytes_read != 0 {
                // UNWRAP: Need to justify this one.
                // Should be safe because we've already done length checks, but I want
                // to spend some more time thinking about it.
                self.buffer
                    .set_memory(head_index, bytes_read as usize, 0)
                    .unwrap();
                self.buffer
                    .put_i64_ordered(self.head_position_index, head + i64::from(bytes_read))
                    .unwrap();
            }
        };
        result.map(|_| cleanup()).map_err(|e| {
            cleanup();
            e
        })?;

        Ok(messages_read)
    }

    /// Read messages from the ring buffer and dispatch to `handler`, up to `message_count_limit`
    /// The handler is given the message type identifier and message body as arguments.
    ///
    /// NOTE: The C++ API will stop reading and clean up if an exception is thrown in the handler
    /// function; by contrast, the Rust API makes no attempt to catch panics and currently
    /// has no way of stopping reading once started.
    pub fn read<F>(&mut self, handler: F) -> Result<usize>
    where
        F: FnMut(i32, &[u8]) -> (),
    {
        self.read_n(handler, usize::max_value())
    }

    /// Claim capacity for a specific message size in the ring buffer. Returns the offset/index
    /// at which to start writing the next record.
    fn claim_capacity(&mut self, required: IndexT) -> Result<IndexT> {
        // QUESTION: Is this mask how we handle the "ring" in ring buffer?
        // Would explain why we assert buffer capacity is a power of two during initialization
        let mask: IndexT = self.capacity - 1;

        // UNWRAP: Known-valid offset calculated during initialization
        let mut head = self
            .buffer
            .get_i64_volatile(self.head_cache_position_index)
            .unwrap();

        let mut tail: i64;
        let mut tail_index: IndexT;
        let mut padding: IndexT;
        // Note the braces, making this a do-while loop
        while {
            tail = self.buffer.get_i64_volatile(self.tail_position_index)?;
            let available_capacity = self.capacity - (tail - head) as IndexT;

            if required > available_capacity {
                head = self.buffer.get_i64_volatile(self.head_position_index)?;

                if required > (self.capacity - (tail - head) as IndexT) {
                    return Ok(INSUFFICIENT_CAPACITY);
                }

                self.buffer
                    .put_i64_ordered(self.head_cache_position_index, head)?;
            }

            padding = 0;

            // Because we assume `tail` and `mask` are always positive integers,
            // it's "safe" to widen the types and bitmask below. We're just trying
            // to imitate C++ here.
            tail_index = (tail & i64::from(mask)) as IndexT;
            let to_buffer_end_length = self.capacity - tail_index;

            if required > to_buffer_end_length {
                let mut head_index = (head & i64::from(mask)) as IndexT;

                if required > head_index {
                    head = self.buffer.get_i64_volatile(self.head_position_index)?;
                    head_index = (head & i64::from(mask)) as IndexT;

                    if required > head_index {
                        return Ok(INSUFFICIENT_CAPACITY);
                    }

                    self.buffer
                        .put_i64_ordered(self.head_cache_position_index, head)?;
                }

                padding = to_buffer_end_length;
            }

            !self.buffer.compare_and_set_i64(
                self.tail_position_index,
                tail,
                tail + i64::from(required) + i64::from(padding),
            )?
        } {}

        if padding != 0 {
            self.buffer.put_i64_ordered(
                tail_index,
                record_descriptor::make_header(padding, record_descriptor::PADDING_MSG_TYPE_ID),
            )?;
            tail_index = 0;
        }

        Ok(tail_index)
    }

    fn check_msg_length(&self, length: IndexT) -> Result<()> {
        if length > self.max_msg_length {
            Err(AeronError::IllegalArgument)
        } else {
            Ok(())
        }
    }

    /// Return the largest possible message size for this buffer
    pub fn max_msg_length(&self) -> IndexT {
        self.max_msg_length
    }

    /// Return the last heartbeat timestamp associated with the consumer of this queue.
    /// Timestamps are milliseconds since 1 Jan 1970, UTC.
    pub fn consumer_heartbeat_time(&self) -> i64 {
        // UNWRAP: Known-valid offset calculated during initialization
        self.buffer
            .get_i64_volatile(self.consumer_heartbeat_index)
            .unwrap()
    }
}

impl<A> Deref for ManyToOneRingBuffer<A>
where
    A: AtomicBuffer,
{
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<A> DerefMut for ManyToOneRingBuffer<A>
where
    A: AtomicBuffer,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

#[cfg(test)]
mod tests {
    use crate::concurrent::ringbuffer::ManyToOneRingBuffer;
    use crate::concurrent::AtomicBuffer;

    const BUFFER_SIZE: usize = 512 + super::buffer_descriptor::TRAILER_LENGTH as usize;

    #[test]
    fn claim_capacity_owned() {
        let mut ring_buf = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SIZE]).unwrap();

        ring_buf.claim_capacity(16).unwrap();
        assert_eq!(
            ring_buf
                .buffer
                .get_i64_volatile(ring_buf.tail_position_index),
            Ok(16)
        );

        let write_start = ring_buf.claim_capacity(16).unwrap();
        assert_eq!(write_start, 16);
    }

    #[test]
    fn claim_capacity_shared() {
        let buf = &mut [0u8; BUFFER_SIZE][..];
        let mut ring_buf = ManyToOneRingBuffer::new(buf).unwrap();

        ring_buf.claim_capacity(16).unwrap();
        assert_eq!(
            ring_buf
                .buffer
                .get_i64_volatile(ring_buf.tail_position_index),
            Ok(16)
        );

        let write_start = ring_buf.claim_capacity(16).unwrap();
        assert_eq!(write_start, 16);
    }
}
