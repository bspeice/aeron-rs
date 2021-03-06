//! Various utility and helper bits for the Aeron client. Predominantly helpful
//! in mapping between concepts in the C++ API and Rust

/// Helper type to indicate indexing operations in Aeron, Synonymous with the
/// Aeron C++ `index_t` type. Used to imitate the Java API.
// QUESTION: Can this just be updated to be `usize` in Rust?
pub type IndexT = i32;

/// Error types from operations in the Aeron client. Synonymous with the exceptions
/// generated by the C++ client.
#[derive(Debug, PartialEq)]
pub enum AeronError {
    /// Indication that an argument provided is an illegal value
    IllegalArgument,
    /// Indication that a memory access would exceed the allowable bounds
    OutOfBounds,
    /// Indication that a buffer operation could not complete because of space constraints
    InsufficientCapacity,
    /// Indication that we have reached an invalid state and can't continue processing
    IllegalState,
}

/// Result type for operations in the Aeron client
pub type Result<T> = ::std::result::Result<T, AeronError>;

/// Bit-level utility functions
pub mod bit {
    use crate::util::IndexT;

    /// Length of the data blocks used by the CPU cache sub-system in bytes
    pub const CACHE_LINE_LENGTH: usize = 64;

    /// Helper method for quick verification that `IndexT` is a positive power of two
    ///
    /// ```rust
    /// # use aeron_rs::util::bit::is_power_of_two;
    /// assert!(is_power_of_two(16));
    /// assert!(!is_power_of_two(17));
    /// ```
    pub fn is_power_of_two(idx: IndexT) -> bool {
        idx > 0 && (idx as u32).is_power_of_two()
    }

    /// Align a `usize` value to the next highest multiple.
    ///
    /// ```rust
    /// # use aeron_rs::util::bit::align;
    /// assert_eq!(align(7, 8), 8);
    ///
    /// // Not intended for alignments that aren't powers of two
    /// assert_eq!(align(52, 12), 52);
    /// assert_eq!(align(52, 16), 64);
    /// ```
    pub const fn align(val: usize, alignment: usize) -> usize {
        (val + (alignment - 1)) & !(alignment - 1)
    }
}
