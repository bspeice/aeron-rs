/// Tests based on the C++ tests included with Aeron
use aeron_rs::client::concurrent::ringbuffer::{
    buffer_descriptor, record_descriptor, ManyToOneRingBuffer,
};
use aeron_rs::client::concurrent::AtomicBuffer;
use aeron_rs::util::bit::align;
use aeron_rs::util::IndexT;
use std::ops::Deref;

const CAPACITY: usize = 1024;
const BUFFER_SZ: usize = CAPACITY + buffer_descriptor::TRAILER_LENGTH as usize;
const ODD_BUFFER_SZ: usize = (CAPACITY - 1) + buffer_descriptor::TRAILER_LENGTH as usize;

const MSG_TYPE_ID: i32 = 101;
const HEAD_COUNTER_INDEX: IndexT = 1024 as IndexT + buffer_descriptor::HEAD_POSITION_OFFSET;
const TAIL_COUNTER_INDEX: IndexT = 1024 as IndexT + buffer_descriptor::TAIL_POSITION_OFFSET;

#[test]
fn should_calculate_capacity_for_buffer() {
    let buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();

    assert_eq!(AtomicBuffer::capacity(buffer.deref()), BUFFER_SZ as IndexT);
    assert_eq!(buffer.capacity(), CAPACITY as IndexT);
}

#[test]
fn should_throw_for_capacity_not_power_of_two() {
    let buffer = ManyToOneRingBuffer::new(vec![0u8; ODD_BUFFER_SZ]);

    assert!(buffer.is_err());
}

#[test]
fn should_throw_when_max_message_size_exceeded() {
    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();

    let bytes = vec![0u8; buffer.max_msg_length() as usize + 1];
    let write_res = buffer.write(MSG_TYPE_ID, &bytes, 0, bytes.len() as IndexT);

    assert!(write_res.is_err());
}

#[test]
fn should_write_to_empty_buffer() {
    let tail: IndexT = 0;
    let tail_index: IndexT = 0;
    let length: IndexT = 8;
    let record_length: IndexT = length + record_descriptor::HEADER_LENGTH;
    let src_index: IndexT = 0;
    let aligned_record_length: IndexT = align(
        record_length as usize,
        record_descriptor::ALIGNMENT as usize,
    ) as IndexT;

    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();
    let src_bytes = vec![0u8; BUFFER_SZ];

    assert!(buffer
        .write(MSG_TYPE_ID, &src_bytes, src_index, length)
        .unwrap());

    assert_eq!(
        buffer.get_i32(record_descriptor::length_offset(tail_index)),
        Ok(record_length)
    );
    assert_eq!(
        buffer.get_i32(record_descriptor::type_offset(tail_index)),
        Ok(MSG_TYPE_ID)
    );
    assert_eq!(
        buffer.get_i64(TAIL_COUNTER_INDEX),
        Ok((tail + aligned_record_length) as i64)
    );
}

#[test]
fn should_reject_write_when_insufficient_space() {
    let length: IndexT = 100;
    let head: IndexT = 0;
    let tail: IndexT = head
        + (CAPACITY
            - align(
                (length - record_descriptor::ALIGNMENT) as usize,
                record_descriptor::ALIGNMENT as usize,
            )) as IndexT;
    let src_index: IndexT = 0;

    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();
    buffer.put_i64(HEAD_COUNTER_INDEX, head as i64).unwrap();
    buffer.put_i64(TAIL_COUNTER_INDEX, tail as i64).unwrap();

    let src_bytes = vec![0u8; BUFFER_SZ];
    let write_res = buffer.write(MSG_TYPE_ID, &src_bytes, src_index, length);

    assert_eq!(write_res, Ok(false));
    assert_eq!(buffer.get_i64(TAIL_COUNTER_INDEX), Ok(tail as i64));
}

#[test]
fn should_reject_write_when_buffer_full() {
    let length: IndexT = 8;
    let head: IndexT = 0;
    let tail: IndexT = head + CAPACITY as IndexT;
    let src_index: IndexT = 0;

    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();
    buffer.put_i64(HEAD_COUNTER_INDEX, head as i64).unwrap();
    buffer.put_i64(TAIL_COUNTER_INDEX, tail as i64).unwrap();

    let src_bytes = vec![0u8; BUFFER_SZ];
    let write_res = buffer.write(MSG_TYPE_ID, &src_bytes, src_index, length);
    assert_eq!(write_res, Ok(false));
    assert_eq!(buffer.get_i64(TAIL_COUNTER_INDEX), Ok(tail as i64));
}

#[test]
fn should_insert_padding_record_plus_message_on_buffer_wrap() {
    let length: IndexT = 100;
    let record_length: IndexT = length + record_descriptor::HEADER_LENGTH;
    let aligned_record_length = align(
        record_length as usize,
        record_descriptor::ALIGNMENT as usize,
    ) as IndexT;
    let tail: IndexT = CAPACITY as IndexT - record_descriptor::ALIGNMENT;
    let head: IndexT = tail - (record_descriptor::ALIGNMENT * 4);
    let src_index: IndexT = 0;

    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();
    buffer.put_i64(HEAD_COUNTER_INDEX, head as i64).unwrap();
    buffer.put_i64(TAIL_COUNTER_INDEX, tail as i64).unwrap();

    let src_bytes = vec![0u8; BUFFER_SZ];
    let write_res = buffer.write(MSG_TYPE_ID, &src_bytes, src_index, length);
    assert_eq!(write_res, Ok(true));

    assert_eq!(
        buffer.get_i32(record_descriptor::type_offset(tail)),
        Ok(record_descriptor::PADDING_MSG_TYPE_ID)
    );
    assert_eq!(
        buffer.get_i32(record_descriptor::length_offset(tail)),
        Ok(record_descriptor::ALIGNMENT)
    );

    assert_eq!(
        buffer.get_i32(record_descriptor::length_offset(0)),
        Ok(record_length)
    );
    assert_eq!(
        buffer.get_i32(record_descriptor::type_offset(0)),
        Ok(MSG_TYPE_ID)
    );
    assert_eq!(
        buffer.get_i64(TAIL_COUNTER_INDEX),
        Ok((tail + aligned_record_length + record_descriptor::ALIGNMENT) as i64)
    );
}

#[test]
fn should_insert_padding_record_plus_message_on_buffer_wrap_with_head_equal_to_tail() {
    let length: IndexT = 100;
    let record_length: IndexT = length + record_descriptor::HEADER_LENGTH;
    let aligned_record_length: IndexT = align(
        record_length as usize,
        record_descriptor::ALIGNMENT as usize,
    ) as IndexT;
    let tail: IndexT = CAPACITY as IndexT - record_descriptor::ALIGNMENT;
    let head: IndexT = tail;
    let src_index: IndexT = 0;

    let mut buffer = ManyToOneRingBuffer::new(vec![0u8; BUFFER_SZ]).unwrap();
    buffer.put_i64(HEAD_COUNTER_INDEX, head as i64).unwrap();
    buffer.put_i64(TAIL_COUNTER_INDEX, tail as i64).unwrap();

    let src_bytes = vec![0u8; BUFFER_SZ];
    let write_res = buffer.write(MSG_TYPE_ID, &src_bytes, src_index, length);
    assert_eq!(write_res, Ok(true));

    assert_eq!(
        buffer.get_i32(record_descriptor::type_offset(tail)),
        Ok(record_descriptor::PADDING_MSG_TYPE_ID)
    );
    assert_eq!(
        buffer.get_i32(record_descriptor::length_offset(tail)),
        Ok(record_descriptor::ALIGNMENT)
    );

    assert_eq!(
        buffer.get_i32(record_descriptor::length_offset(0)),
        Ok(record_length)
    );
    assert_eq!(
        buffer.get_i32(record_descriptor::type_offset(0)),
        Ok(MSG_TYPE_ID)
    );
    assert_eq!(
        buffer.get_i64(TAIL_COUNTER_INDEX),
        Ok((tail + aligned_record_length + record_descriptor::ALIGNMENT) as i64)
    );
}
