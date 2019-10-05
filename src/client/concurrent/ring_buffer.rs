//! Ring buffer wrapper for communicating with the Media Driver
use crate::client::concurrent::atomic_buffer::AtomicBuffer;
use crate::util::{AeronError, IndexT, Result};

/// Description of the Ring Buffer schema.
pub mod buffer_descriptor {
    use crate::client::concurrent::atomic_buffer::AtomicBuffer;
    use crate::util::AeronError::IllegalArgument;
    use crate::util::{is_power_of_two, IndexT, Result, CACHE_LINE_LENGTH};

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
    pub fn check_capacity(buffer: &AtomicBuffer<'_>) -> Result<IndexT> {
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
pub mod record_descriptor {
    use crate::util::IndexT;
    use std::mem::size_of;

    /// Size of the ring buffer record header.
    pub const HEADER_LENGTH: IndexT = size_of::<i32>() as IndexT * 2;

    /// Message type indicating to the media driver that space has been reserved,
    /// and is not yet ready for processing.
    pub const PADDING_MSG_TYPE_ID: i32 = -1;

    /// Retrieve the header bits for a ring buffer record.
    pub fn make_header(length: i32, msg_type_id: i32) -> i64 {
        // QUESTION: Instead of masking, can't we just cast and return u32/u64?
        // Smells like Java.
        ((i64::from(msg_type_id) & 0xFFFF_FFFF) << 32) | (i64::from(length) & 0xFFFF_FFFF)
    }
}

/// Multi-producer, single-consumer ring buffer implementation.
pub struct ManyToOneRingBuffer<'a> {
    buffer: AtomicBuffer<'a>,
    capacity: IndexT,
    tail_position_index: IndexT,
    head_cache_position_index: IndexT,
    head_position_index: IndexT,
    _correlation_id_counter_index: IndexT,
}

impl<'a> ManyToOneRingBuffer<'a> {
    /// Create a many-to-one ring buffer from an underlying atomic buffer.
    pub fn wrap(buffer: AtomicBuffer<'a>) -> Result<Self> {
        buffer_descriptor::check_capacity(&buffer).map(|capacity| ManyToOneRingBuffer {
            buffer,
            capacity,
            tail_position_index: capacity + buffer_descriptor::TAIL_POSITION_OFFSET,
            head_cache_position_index: capacity + buffer_descriptor::HEAD_CACHE_POSITION_OFFSET,
            head_position_index: capacity + buffer_descriptor::HEAD_POSITION_OFFSET,
            _correlation_id_counter_index: capacity + buffer_descriptor::CORRELATION_COUNTER_OFFSET,
        })
    }

    /// Claim capacity for a specific message size in the ring buffer. Returns the offset/index
    /// at which to start writing the next record.
    // TODO: Shouldn't be `pub`, just trying to avoid warnings
    pub fn claim_capacity(&mut self, required: IndexT) -> Result<IndexT> {
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

            println!("Available: {}", available_capacity);
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

            println!("To buffer end: {}", to_buffer_end_length);
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
}

#[cfg(test)]
mod tests {
    use crate::client::concurrent::atomic_buffer::AtomicBuffer;
    use crate::client::concurrent::ring_buffer::ManyToOneRingBuffer;

    #[test]
    fn basic_claim_space() {
        let buf_size = super::buffer_descriptor::TRAILER_LENGTH as usize + 64;
        let mut buf = vec![0u8; buf_size];

        let atomic_buf = AtomicBuffer::wrap(&mut buf);
        let mut ring_buf = ManyToOneRingBuffer::wrap(atomic_buf).unwrap();

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
