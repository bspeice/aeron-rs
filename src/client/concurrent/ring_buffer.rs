//! Ring buffer wrapper for communicating with the Media Driver
use crate::client::concurrent::atomic_buffer::AtomicBuffer;
use crate::util::{IndexT, Result};

/// Description of the Ring Buffer schema. Each Ring Buffer looks like:
///
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                        Buffer Data                          ...
/// ...                                                             |
/// +---------------------------------------------------------------+
/// |                                                               |
/// |                       Tail Position                           |
/// |                                                               |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                                                               |
/// |                    Head Cache Position                        |
/// |                                                               |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                                                               |
/// |                       Head Position                           |
/// |                                                               |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                                                               |
/// |                   Correlation ID Counter                      |
/// |                                                               |
/// |                                                               |
/// +---------------------------------------------------------------+
/// |                                                               |
/// |                     Consumer Heartbeat                        |
/// |                                                               |
/// |                                                               |
/// +---------------------------------------------------------------+
/// ```
pub mod descriptor {
    use crate::client::concurrent::atomic_buffer::AtomicBuffer;
    use crate::util::AeronError::IllegalArgument;
    use crate::util::{is_power_of_two, IndexT, Result, CACHE_LINE_LENGTH};

    /// Offset of the correlation id counter, as measured in bytes past
    /// the start of the ring buffer metadata trailer
    pub const CORRELATION_COUNTER_OFFSET: usize = CACHE_LINE_LENGTH * 8;

    /// Total size of the ring buffer metadata trailer
    pub const TRAILER_LENGTH: usize = CACHE_LINE_LENGTH * 12;

    /// Verify the capacity of a buffer is legal for use as a ring buffer.
    /// Returns the actual buffer capacity once ring buffer metadata has been removed.
    pub fn check_capacity(buffer: &AtomicBuffer<'_>) -> Result<IndexT> {
        let capacity = (buffer.len() - TRAILER_LENGTH) as IndexT;
        if is_power_of_two(capacity) {
            Ok(capacity)
        } else {
            Err(IllegalArgument)
        }
    }
}

/// Multi-producer, single-consumer ring buffer implementation.
pub struct ManyToOneRingBuffer<'a> {
    _buffer: AtomicBuffer<'a>,
    _capacity: IndexT,
    _correlation_counter_offset: IndexT,
}

impl<'a> ManyToOneRingBuffer<'a> {
    /// Create a many-to-one ring buffer from an underlying atomic buffer
    pub fn wrap(buffer: AtomicBuffer<'a>) -> Result<Self> {
        descriptor::check_capacity(&buffer).map(|capacity| ManyToOneRingBuffer {
            _buffer: buffer,
            _capacity: capacity,
            _correlation_counter_offset: capacity
                + descriptor::CORRELATION_COUNTER_OFFSET as IndexT,
        })
    }
}
