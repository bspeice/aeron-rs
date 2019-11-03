//! Read messages that are broadcast from the media driver; this is the primary means
//! of receiving data.
use crate::client::concurrent::AtomicBuffer;
use crate::util::bit::align;
use crate::util::{AeronError, IndexT, Result};
use std::sync::atomic::{AtomicI64, Ordering};

/// Description of the broadcast buffer schema
pub mod buffer_descriptor {
    use crate::util::bit::{is_power_of_two, CACHE_LINE_LENGTH};
    use crate::util::{AeronError, IndexT, Result};
    use std::mem::size_of;

    /// Offset within the trailer for the tail intended value
    pub const TAIL_INTENT_COUNTER_OFFSET: IndexT = 0;

    /// Offset within the trailer for the tail value
    pub const TAIL_COUNTER_OFFSET: IndexT = TAIL_INTENT_COUNTER_OFFSET + size_of::<i64>() as IndexT;

    /// Offset within the buffer trailer for the latest sequence value
    pub const LATEST_COUNTER_OFFSET: IndexT = TAIL_COUNTER_OFFSET + size_of::<i64>() as IndexT;

    /// Size of the broadcast buffer metadata trailer
    pub const TRAILER_LENGTH: IndexT = CACHE_LINE_LENGTH as IndexT * 2;

    pub(super) fn check_capacity(capacity: IndexT) -> Result<()> {
        // QUESTION: Why does C++ throw IllegalState here?
        // Would've expected it to throw IllegalArgument like ring buffer
        if is_power_of_two(capacity) {
            Ok(())
        } else {
            Err(AeronError::IllegalArgument)
        }
    }
}

/// Broadcast buffer message header
// QUESTION: Isn't this the same as the ring buffer descriptor?
// Why not consolidate them?
pub mod record_descriptor {
    use crate::util::IndexT;

    /// Message type to indicate a record used only
    /// for padding the buffer
    pub const PADDING_MSG_TYPE_ID: i32 = -1;

    /// Offset from the beginning of a record to its length
    pub const LENGTH_OFFSET: IndexT = 0;

    /// Offset from the beginning of a record to its type
    pub const TYPE_OFFSET: IndexT = 4;

    /// Total header length for each record
    pub const HEADER_LENGTH: IndexT = 8;

    /// Alignment for all broadcast records
    pub const RECORD_ALIGNMENT: IndexT = HEADER_LENGTH;

    /// Retrieve the byte offset for a record's length field given the record start
    pub fn length_offset(record_offset: IndexT) -> IndexT {
        record_offset + LENGTH_OFFSET
    }

    /// Retrieve the byte offset for a record's type field given the record start
    pub fn type_offset(record_offset: IndexT) -> IndexT {
        record_offset + TYPE_OFFSET
    }

    /// Retrieve the byte offset for a record's message given the record start
    pub fn msg_offset(record_offset: IndexT) -> IndexT {
        record_offset + HEADER_LENGTH
    }
}

/// Receive messages from a transmission stream. Works by polling `receive_next`
/// until `true` is returned, then inspecting messages using the provided methods.
pub struct BroadcastReceiver<A>
where
    A: AtomicBuffer,
{
    buffer: A,
    capacity: IndexT,
    mask: IndexT,
    tail_intent_counter_index: IndexT,
    tail_counter_index: IndexT,
    latest_counter_index: IndexT,
    record_offset: IndexT,
    cursor: i64,
    next_record: i64,
    lapped_count: AtomicI64,
}

impl<A> BroadcastReceiver<A>
where
    A: AtomicBuffer,
{
    /// Create a new receiver backed by `buffer`
    pub fn new(buffer: A) -> Result<Self> {
        let capacity = buffer.capacity() - buffer_descriptor::TRAILER_LENGTH;
        buffer_descriptor::check_capacity(capacity)?;
        let mask = capacity - 1;

        let latest_counter_index = capacity + buffer_descriptor::LATEST_COUNTER_OFFSET;
        let cursor = buffer.get_i64(latest_counter_index)?;

        Ok(BroadcastReceiver {
            buffer,
            capacity,
            mask,
            tail_intent_counter_index: capacity + buffer_descriptor::TAIL_INTENT_COUNTER_OFFSET,
            tail_counter_index: capacity + buffer_descriptor::TAIL_COUNTER_OFFSET,
            latest_counter_index,
            record_offset: (cursor as i32) & mask,
            cursor,
            next_record: cursor,
            lapped_count: AtomicI64::new(0),
        })
    }

    /// Get the total capacity of this broadcast receiver
    pub fn capacity(&self) -> IndexT {
        self.capacity
    }

    /// Get the number of times the transmitter has lapped this receiver. Each lap
    /// represents at least a buffer's worth of lost data.
    pub fn lapped_count(&self) -> i64 {
        // QUESTION: C++ just uses `std::atomic<long>`, what are the ordering semantics?
        // For right now I'm just assuming it's sequentially consistent
        self.lapped_count.load(Ordering::SeqCst)
    }

    /// Non-blocking receive of next message from the transmission stream.
    /// If loss has occurred, `lapped_count` will be incremented. Returns `true`
    /// if the next transmission is available, `false` otherwise.
    pub fn receive_next(&mut self) -> Result<bool> {
        let mut is_available = false;
        let tail: i64 = self.buffer.get_i64_volatile(self.tail_counter_index)?;
        let mut cursor: i64 = self.next_record;

        if tail > cursor {
            // NOTE: C++ and Java clients do these first lines slightly differently. As far
            // as I can tell, this is structurally equivalent, and Clippy yells less at me.
            if !self._validate(cursor) {
                self.lapped_count.fetch_add(1, Ordering::SeqCst);
                cursor = self.buffer.get_i64(self.latest_counter_index)?;
            }
            let mut record_offset = (cursor as i32) & self.mask;

            self.cursor = cursor;
            self.next_record = cursor
                + align(
                    self.buffer
                        .get_i32(record_descriptor::length_offset(record_offset))?
                        as usize,
                    record_descriptor::RECORD_ALIGNMENT as usize,
                ) as i64;

            if record_descriptor::PADDING_MSG_TYPE_ID
                == self
                    .buffer
                    .get_i32(record_descriptor::type_offset(record_offset))?
            {
                record_offset = 0;
                self.cursor = self.next_record;
                self.next_record += align(
                    self.buffer
                        .get_i32(record_descriptor::length_offset(record_offset))?
                        as usize,
                    record_descriptor::RECORD_ALIGNMENT as usize,
                ) as i64;
            }

            self.record_offset = record_offset;
            is_available = true;
        }

        Ok(is_available)
    }

    /// Get the length of the message in the current record
    pub fn length(&self) -> Result<i32> {
        Ok(self
            .buffer
            .get_i32(record_descriptor::length_offset(self.record_offset))?
            - record_descriptor::HEADER_LENGTH)
    }

    /// Get the offset to the message content in the current record
    pub fn offset(&self) -> i32 {
        record_descriptor::msg_offset(self.record_offset)
    }

    /// Ensure that the current received record is still valid and has not been
    /// overwritten.
    pub fn validate(&self) -> bool {
        // QUESTION: C++ uses `atomic::acquire()` here, what does that do?
        self._validate(self.cursor)
    }

    /// Get the message type identifier for the current record
    pub fn msg_type_id(&self) -> Result<i32> {
        Ok(self
            .buffer
            .get_i32(record_descriptor::type_offset(self.record_offset))?)
    }

    fn _validate(&self, cursor: i64) -> bool {
        // UNWRAP: Length checks performed during initialization
        (cursor + i64::from(self.capacity))
            > self
                .buffer
                .get_i64_volatile(self.tail_intent_counter_index)
                .unwrap()
    }
}

/// Broadcast receiver that copies messages to an internal buffer.
///
/// The benefit of copying every message is that we keep a consistent view of the data
/// even if we're lapped while reading. However, this may be overkill if you can
/// guarantee the stream never outpaces you.
pub struct CopyBroadcastReceiver<A>
where
    A: AtomicBuffer,
{
    receiver: BroadcastReceiver<A>,
    scratch: Vec<u8>,
}

impl<A> CopyBroadcastReceiver<A>
where
    A: AtomicBuffer,
{
    /// Create a new broadcast receiver
    pub fn new(receiver: BroadcastReceiver<A>) -> Self {
        CopyBroadcastReceiver {
            receiver,
            scratch: Vec::with_capacity(4096),
        }
    }

    /// Attempt to receive a single message from the broadcast buffer,
    /// and deliver it to the message handler if successful.
    /// Returns the number of messages received.
    pub fn receive<F>(&mut self, mut handler: F) -> Result<i32>
    where
        F: FnMut(i32, &[u8]) -> (),
    {
        let mut messages_received = 0;
        let last_seen_lapped_count = self.receiver.lapped_count();

        if self.receiver.receive_next()? {
            if last_seen_lapped_count != self.receiver.lapped_count() {
                // The C++ API uses IllegalArgument here, but returns IllegalState
                // with the same message later.
                return Err(AeronError::IllegalState);
            }

            let length = self.receiver.length()?;
            if length > AtomicBuffer::capacity(&self.scratch) {
                return Err(AeronError::IllegalState);
            }

            let msg_type_id = self.receiver.msg_type_id()?;
            self.scratch
                .put_bytes(0, &self.receiver.buffer, self.receiver.offset(), length)?;

            if !self.receiver.validate() {
                return Err(AeronError::IllegalState);
            }
            handler(msg_type_id, &self.scratch[0..length as usize]);
            messages_received += 1;
        }

        Ok(messages_received)
    }
}
