//! [Aeron](https://github.com/real-logic/aeron) client for Rust
#![deny(missing_docs)]

pub mod client;
pub mod control_protocol;
pub mod driver;

const fn sematic_version_compose(major: u8, minor: u8, patch: u8) -> i32 {
    (major as i32) << 16 | (minor as i32) << 8 | (patch as i32)
}

#[cfg(test)]
mod tests {
    use crate::sematic_version_compose;

    #[test]
    fn version_compose_cnc() {
        assert_eq!(sematic_version_compose(0, 0, 16), 16);
    }
}
