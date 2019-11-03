use aeron_rs::client::concurrent::broadcast::{buffer_descriptor, BroadcastReceiver};
use aeron_rs::util::IndexT;

const CAPACITY: usize = 1024;
const TOTAL_BUFFER_LENGTH: usize = CAPACITY + buffer_descriptor::TRAILER_LENGTH as usize;

// NOTE: The C++ tests use a mock atomic buffer for testing to validate behavior.
// This is rather hard to do with Rust, so we skip behavior validation for now,
// and assume that other tests will end up verifying needed behavior.

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
