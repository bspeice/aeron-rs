//! Ring buffer wrapper for communicating with the Media Driver
use crate::client::concurrent::AtomicBuffer;
use crate::util::bit::align;
use crate::util::{bit, AeronError, IndexT, Result};
use std::ops::Deref;

/// Description of the Ring Buffer schema.
pub mod buffer_descriptor {
    use crate::client::concurrent::AtomicBuffer;
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

    /// Total size of the ring buffer metadata trailer.
    pub const TRAILER_LENGTH: IndexT = (CACHE_LINE_LENGTH * 12) as IndexT;

    /// Verify the capacity of a buffer is legal for use as a ring buffer.
    /// Returns the actual capacity excluding ring buffer metadata.
    pub fn check_capacity<A>(buffer: &A) -> Result<IndexT>
    where
        A: AtomicBuffer,
    {
        let capacity = (buffer.len() - TRAILER_LENGTH as usize) as IndexT;
        if is_power_of_two(capacity) {
            Ok(capacity)
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

    pub(super) fn length_offset(record_offset: IndexT) -> IndexT {
        record_offset
    }

    pub(super) fn record_length(header: i64) -> i32 {
        header as i32
    }

    pub(super) fn message_type_id(header: i64) -> i32 {
        (header >> 32) as i32
    }
}

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
}

impl<A> ManyToOneRingBuffer<A>
where
    A: AtomicBuffer,
{
    /// Create a many-to-one ring buffer from an underlying atomic buffer.
    pub fn new(buffer: A) -> Result<Self> {
        buffer_descriptor::check_capacity(&buffer).map(|capacity| ManyToOneRingBuffer {
            buffer,
            capacity,
            max_msg_length: capacity / 8,
            tail_position_index: capacity + buffer_descriptor::TAIL_POSITION_OFFSET,
            head_cache_position_index: capacity + buffer_descriptor::HEAD_CACHE_POSITION_OFFSET,
            head_position_index: capacity + buffer_descriptor::HEAD_POSITION_OFFSET,
            correlation_id_counter_index: capacity + buffer_descriptor::CORRELATION_COUNTER_OFFSET,
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

    /// Write a message into the ring buffer
    pub fn write<B>(
        &mut self,
        msg_type_id: i32,
        source: &B,
        source_index: IndexT,
        length: IndexT,
    ) -> Result<()>
    where
        B: AtomicBuffer,
    {
        record_descriptor::check_msg_type_id(msg_type_id)?;
        self.check_msg_length(length)?;

        let record_len = length + record_descriptor::HEADER_LENGTH;
        let required = bit::align(record_len as usize, record_descriptor::ALIGNMENT as usize);
        let record_index = self.claim_capacity(required as IndexT)?;

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

        Ok(())
    }

    /*
    /// Read messages from the ring buffer and dispatch to `handler`, up to `message_count_limit`
    pub fn read<F>(&mut self, mut handler: F, message_count_limit: usize) -> Result<usize>
    where
        F: FnMut(i32, &A, IndexT, IndexT) -> (),
    {
        // UNWRAP: Bounds check performed during buffer creation
        let head = self.buffer.get_i64(self.head_position_index).unwrap();
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
                handler(
                    msg_type_id,
                    &self.buffer,
                    record_descriptor::encoded_msg_offset(record_index),
                    record_length - record_descriptor::HEADER_LENGTH,
                );
            }
            Ok(())
        })();

        // C++ has much better semantics for handling cleanup like this; however, because
        // it would require us to capture a mutable reference to self, it's not feasible
        // in Rust (since the main operation also needs mutable access to self).
        let mut cleanup = || {
            if bytes_read != 0 {
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
    */

    /// Claim capacity for a specific message size in the ring buffer. Returns the offset/index
    /// at which to start writing the next record.
    fn claim_capacity(&mut self, required: IndexT) -> Result<IndexT> {
        // QUESTION: Is this mask how we handle the "ring" in ring buffer?
        // Would explain why we assert buffer capacity is a power of two during initialization
        let mask = self.capacity - 1;

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
            // UNWRAP: Known-valid offset calculated during initialization
            tail = self
                .buffer
                .get_i64_volatile(self.tail_position_index)
                .unwrap();
            let available_capacity = self.capacity - (tail - head) as IndexT;

            if required > available_capacity {
                // UNWRAP: Known-valid offset calculated during initialization
                head = self
                    .buffer
                    .get_i64_volatile(self.head_position_index)
                    .unwrap();

                if required > (self.capacity - (tail - head) as IndexT) {
                    return Err(AeronError::InsufficientCapacity);
                }

                // UNWRAP: Known-valid offset calculated during initialization
                self.buffer
                    .put_i64_ordered(self.head_cache_position_index, head)
                    .unwrap();
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
                    // UNWRAP: Known-valid offset calculated during initialization
                    head = self
                        .buffer
                        .get_i64_volatile(self.head_position_index)
                        .unwrap();
                    head_index = (head & i64::from(mask)) as IndexT;

                    if required > head_index {
                        return Err(AeronError::InsufficientCapacity);
                    }

                    // UNWRAP: Known-valid offset calculated during initialization
                    self.buffer
                        .put_i64_ordered(self.head_cache_position_index, head)
                        .unwrap();
                }

                padding = to_buffer_end_length;
            }

            // UNWRAP: Known-valid offset calculated during initialization
            !self
                .buffer
                .compare_and_set_i64(
                    self.tail_position_index,
                    tail,
                    tail + i64::from(required) + i64::from(padding),
                )
                .unwrap()
        } {}

        if padding != 0 {
            // UNWRAP: Known-valid offset calculated during initialization
            self.buffer
                .put_i64_ordered(
                    tail_index,
                    record_descriptor::make_header(padding, record_descriptor::PADDING_MSG_TYPE_ID),
                )
                .unwrap();
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

#[cfg(test)]
mod tests {
    use crate::client::concurrent::ringbuffer::{
        buffer_descriptor, record_descriptor, ManyToOneRingBuffer,
    };
    use crate::client::concurrent::AtomicBuffer;
    use crate::util::IndexT;
    use std::mem::size_of;

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

    #[test]
    fn write_basic() {
        let mut ring_buffer =
            ManyToOneRingBuffer::new(vec![0u8; BUFFER_SIZE]).expect("Invalid buffer size");

        let source_bytes = &mut [12u8, 0, 0, 0][..];
        let source_len = source_bytes.len() as IndexT;
        let type_id = 1;
        ring_buffer
            .write(type_id, &source_bytes, 0, source_len)
            .unwrap();

        let record_len = source_len + record_descriptor::HEADER_LENGTH;
        assert_eq!(
            ring_buffer.get_i64_volatile(0).unwrap(),
            record_descriptor::make_header(record_len, type_id)
        );
        assert_eq!(
            ring_buffer
                .get_i64_volatile(size_of::<i64>() as IndexT)
                .unwrap(),
            12
        );
    }

    /*
    #[test]
    fn read_basic() {
        // Similar to write basic, put something into the buffer
        let mut bytes = vec![0u8; 512 + buffer_descriptor::TRAILER_LENGTH as usize];
        let buffer = AtomicBuffer::wrap(&mut bytes);
        let mut ring_buffer = ManyToOneRingBuffer::wrap(buffer).expect("Invalid buffer size");

        let mut source_buffer = &mut [12u8, 0, 0, 0, 0, 0, 0, 0][..];
        let source_len = source_bytes.len() as IndexT;
        let type_id = 1;
        ring_buffer
            .write(type_id, &source_buffer, 0, source_len)
            .unwrap();

        // Now we can start the actual read process
        let c = |_, buf: &dyn AtomicBuffer, offset, _| {
            assert_eq!(buf.get_i64_volatile(offset).unwrap(), 12)
        };
        ring_buffer.read(c, 1).unwrap();

        // Make sure that the buffer was zeroed on finish
        drop(ring_buffer);
        let buffer = AtomicBuffer::wrap(&mut bytes);
        for i in (0..record_descriptor::ALIGNMENT * 1).step_by(4) {
            assert_eq!(buffer.get_i32(i).unwrap(), 0);
        }
    }

    #[test]
    fn read_multiple() {
        let mut bytes = vec![0u8; 512 + buffer_descriptor::TRAILER_LENGTH as usize];
        let buffer = AtomicBuffer::wrap(&mut bytes);
        let mut ring_buffer = ManyToOneRingBuffer::wrap(buffer).expect("Invalid buffer size");

        let mut source_buffer = &mut [12u8, 0, 0, 0, 0, 0, 0, 0][..];
        let source_len = source_bytes.len() as IndexT;
        let type_id = 1;
        ring_buffer
            .write(type_id, &source_buffer, 0, source_len)
            .unwrap();
        ring_buffer
            .write(type_id, &source_buffer, 0, source_len)
            .unwrap();

        let mut msg_count = 0;
        let c = |_, buf: &dyn AtomicBuffer, offset, _| {
            msg_count += 1;
            assert_eq!(buf.get_i64_volatile(offset).unwrap(), 12);
        };
        ring_buffer.read(c, 2).unwrap();
        assert_eq!(msg_count, 2);

        // Make sure that the buffer was zeroed on finish
        drop(ring_buffer);
        let buffer = AtomicBuffer::wrap(&mut bytes);
        for i in (0..record_descriptor::ALIGNMENT * 2).step_by(4) {
            assert_eq!(buffer.get_i32(i).unwrap(), 0);
        }
    }
    */
}
