use aeron_rs::concurrent::broadcast::{
    buffer_descriptor, record_descriptor, BroadcastReceiver,
};
use aeron_rs::concurrent::AtomicBuffer;
use aeron_rs::util::bit::align;
use aeron_rs::util::IndexT;

const CAPACITY: usize = 1024;
const TOTAL_BUFFER_LENGTH: usize = CAPACITY + buffer_descriptor::TRAILER_LENGTH as usize;
const MSG_TYPE_ID: i32 = 7;
const TAIL_INTENT_COUNTER_INDEX: i32 =
    CAPACITY as i32 + buffer_descriptor::TAIL_INTENT_COUNTER_OFFSET;
const TAIL_COUNTER_INDEX: i32 = CAPACITY as i32 + buffer_descriptor::TAIL_COUNTER_OFFSET;
const LATEST_COUNTER_INDEX: i32 = CAPACITY as i32 + buffer_descriptor::LATEST_COUNTER_OFFSET;

// NOTE: The C++ tests use a mock atomic buffer for testing to validate behavior.
// I haven't implemented this in Rust mostly because it's a great deal of work,
// but it means we're not verifying that BroadcastReceiver uses the properly
// synchronized method calls on the underlying buffer.

#[test]
fn should_calculate_capacity_for_buffer() {
    let buffer = BroadcastReceiver::new(vec![0u8; TOTAL_BUFFER_LENGTH]).unwrap();
    assert_eq!(buffer.capacity(), CAPACITY as IndexT);
}

#[test]
fn should_throw_exception_for_capacity_that_is_not_power_of_two() {
    let bytes = vec![0u8; 777 + buffer_descriptor::TRAILER_LENGTH as usize];

    assert!(BroadcastReceiver::new(bytes).is_err());
}

#[test]
fn should_not_be_lapped_before_reception() {
    let receiver = BroadcastReceiver::new(vec![0u8; TOTAL_BUFFER_LENGTH]).unwrap();
    assert_eq!(receiver.lapped_count(), 0);
}

#[test]
fn should_not_receive_from_empty_buffer() {
    let mut receiver = BroadcastReceiver::new(vec![0u8; TOTAL_BUFFER_LENGTH]).unwrap();
    assert_eq!(receiver.receive_next(), Ok(false));
}

#[test]
fn should_receive_first_message_from_buffer() {
    let length: i32 = 8;
    let record_length: i32 = length + record_descriptor::HEADER_LENGTH;
    let aligned_record_length: i32 = align(
        record_length as usize,
        record_descriptor::RECORD_ALIGNMENT as usize,
    ) as i32;
    let tail = aligned_record_length as i64;
    let latest_record = tail - aligned_record_length as i64;
    let record_offset = latest_record as i32;

    let mut buffer = vec![0u8; TOTAL_BUFFER_LENGTH];
    buffer.put_i64(TAIL_COUNTER_INDEX, tail).unwrap();
    buffer.put_i64(TAIL_INTENT_COUNTER_INDEX, tail).unwrap();
    buffer
        .put_i32(
            record_descriptor::length_offset(record_offset),
            record_length,
        )
        .unwrap();
    buffer
        .put_i32(record_descriptor::type_offset(record_offset), MSG_TYPE_ID)
        .unwrap();

    let mut receiver = BroadcastReceiver::new(buffer).unwrap();
    assert_eq!(receiver.receive_next(), Ok(true));
    assert_eq!(receiver.msg_type_id(), Ok(MSG_TYPE_ID));
    assert_eq!(
        receiver.offset(),
        record_descriptor::msg_offset(record_offset)
    );
    assert_eq!(receiver.length(), Ok(length));
    assert!(receiver.validate());
}

#[test]
fn should_receive_two_messages_from_buffer() {
    let length: i32 = 8;
    let record_length: i32 = length + record_descriptor::HEADER_LENGTH;
    let aligned_record_length: i32 = align(
        record_length as usize,
        record_descriptor::RECORD_ALIGNMENT as usize,
    ) as i32;
    let tail = (aligned_record_length * 2) as i64;
    let latest_record = tail - aligned_record_length as i64;
    let record_offset_one = 0;
    let record_offset_two = latest_record as i32;

    let mut buffer = vec![0u8; TOTAL_BUFFER_LENGTH];
    buffer.put_i64(TAIL_COUNTER_INDEX, tail).unwrap();
    buffer.put_i64(TAIL_INTENT_COUNTER_INDEX, tail).unwrap();

    buffer
        .put_i32(
            record_descriptor::length_offset(record_offset_one),
            record_length,
        )
        .unwrap();
    buffer
        .put_i32(
            record_descriptor::type_offset(record_offset_one),
            MSG_TYPE_ID,
        )
        .unwrap();

    buffer
        .put_i32(
            record_descriptor::length_offset(record_offset_two),
            record_length,
        )
        .unwrap();
    buffer
        .put_i32(
            record_descriptor::type_offset(record_offset_two),
            MSG_TYPE_ID,
        )
        .unwrap();

    let mut receiver = BroadcastReceiver::new(buffer).unwrap();
    assert_eq!(receiver.receive_next(), Ok(true));
    assert_eq!(receiver.msg_type_id(), Ok(MSG_TYPE_ID));
    assert_eq!(
        receiver.offset(),
        record_descriptor::msg_offset(record_offset_one)
    );
    assert_eq!(receiver.length(), Ok(length));
    assert!(receiver.validate());

    assert_eq!(receiver.receive_next(), Ok(true));
    assert_eq!(receiver.msg_type_id(), Ok(MSG_TYPE_ID));
    assert_eq!(
        receiver.offset(),
        record_descriptor::msg_offset(record_offset_two)
    );
    assert_eq!(receiver.length(), Ok(length));
    assert!(receiver.validate());
}

#[test]
fn should_late_join_transmission() {
    let length: i32 = 8;
    let record_length: i32 = length + record_descriptor::HEADER_LENGTH;
    let aligned_record_length: i32 = align(
        record_length as usize,
        record_descriptor::RECORD_ALIGNMENT as usize,
    ) as i32;
    let tail = (CAPACITY * 3) as i64
        + record_descriptor::HEADER_LENGTH as i64
        + aligned_record_length as i64;
    let latest_record = tail - aligned_record_length as i64;
    let record_offset = latest_record as i32 & (CAPACITY - 1) as i32;

    let mut buffer = vec![0u8; TOTAL_BUFFER_LENGTH];
    // In order to properly do this test, we have to initialize the broadcast receiver
    // while the buffer is empty, and then write into the buffer afterward. Rust is understandably
    // not happy about that, but that's the price we pay for not dealing with mocking.
    let receiver_buffer =
        unsafe { ::std::slice::from_raw_parts_mut(buffer.as_mut_ptr(), buffer.len()) };
    let mut receiver = BroadcastReceiver::new(receiver_buffer).unwrap();

    buffer.put_i64(TAIL_COUNTER_INDEX, tail).unwrap();
    buffer.put_i64(TAIL_INTENT_COUNTER_INDEX, tail).unwrap();
    buffer.put_i64(LATEST_COUNTER_INDEX, latest_record).unwrap();

    buffer
        .put_i32(
            record_descriptor::length_offset(record_offset),
            record_length,
        )
        .unwrap();
    buffer
        .put_i32(record_descriptor::type_offset(record_offset), MSG_TYPE_ID)
        .unwrap();

    assert_eq!(receiver.receive_next(), Ok(true));
    assert_eq!(receiver.msg_type_id(), Ok(MSG_TYPE_ID));
    assert_eq!(
        receiver.offset(),
        record_descriptor::msg_offset(record_offset)
    );
    assert_eq!(receiver.length(), Ok(length));
    assert!(receiver.validate());
    assert!(receiver.lapped_count() > 0);
}

// TODO: Implement the rest of the tests
// Currently not done because of the need to mock the AtomicBuffer
