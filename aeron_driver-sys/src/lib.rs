#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Construct a C-compatible enum out of a set of constants.
/// Commonly used for types in Aeron that have fixed values via `#define`,
/// but aren't actually enums (e.g. AERON_COMMAND_.*, AERON_ERROR_CODE_.*).
/// Behavior is ultimately very similar to `num::FromPrimitive`.
macro_rules! define_enum {
    ($(#[$outer:meta])*, $name:ident, [$(($left:ident, $right:expr)),*]) => {
        #[repr(u32)]
        #[derive(Debug, PartialEq)]
        $(#[$outer])*
        pub enum $name {
            $($left = $right),*
        }

        impl ::std::convert::TryFrom<u32> for $name {
            type Error = ();
            fn try_from(val: u32) -> Result<$name, ()> {
                match val {
                    $(v if v == $name::$left as u32 => Ok($name::$left)),*,
                    _ => Err(())
                }
            }
        }
    }
}

define_enum!(
    #[doc = "Command codes used when interacting with the Media Driver"],
    AeronCommand, [
        (AddPublication, AERON_COMMAND_ADD_PUBLICATION),
        (RemovePublication, AERON_COMMAND_REMOVE_PUBLICATION)
    ]
);

define_enum!(
    #[doc = "Error codes used by the Media Driver control protocol"],
    AeronControlErrorCode, [
        (GenericError, AERON_ERROR_CODE_GENERIC_ERROR)
    ]
);

#[cfg(test)]
mod tests {
    use crate::*;
    use std::convert::TryInto;

    #[test]
    fn version_check() {
        let major = unsafe { crate::aeron_version_major() };
        let minor = unsafe { crate::aeron_version_minor() };
        let patch = unsafe { crate::aeron_version_patch() };
        assert_eq!(major, 1);
        assert_eq!(minor, 21);
        assert_eq!(patch, 2);
    }

    #[test]
    fn define_enum_try() {
        assert_eq!(
            Ok(AeronCommand::AddPublication),
            AERON_COMMAND_ADD_PUBLICATION.try_into()
        );
    }
}
