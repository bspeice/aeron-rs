//! Utilities for interacting with the control protocol of the Media Driver
use aeron_driver_sys::*;

/// Construct a C-compatible enum out of a set of constants.
/// Commonly used for types in Aeron that have fixed values via `#define`,
/// but aren't actually enums (e.g. AERON_COMMAND_.*, AERON_ERROR_CODE_.*).
/// Behavior is ultimately very similar to `num::FromPrimitive`.
macro_rules! define_enum {
    (
        $(#[$outer:meta])*
        pub enum $name:ident {$(
            $(#[$inner:meta]),*
            $left:ident = $right:ident,
        )+}
    ) => {
        #[repr(u32)]
        #[derive(Debug, PartialEq)]
        $(#[$outer])*
        pub enum $name {$(
            $(#[$inner])*
            $left = $right,
        )*}

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
    #[doc = "Commands sent from clients to the Media Driver"]
    pub enum ClientCommand {
        #[doc = "Client declaring a new publication"]
        AddPublication = AERON_COMMAND_ADD_PUBLICATION,
        #[doc = "Client removing a publication"]
        RemovePublication = AERON_COMMAND_REMOVE_PUBLICATION,
    }
);

#[cfg(test)]
mod tests {
    use crate::control_protocol::ClientCommand;
    use std::convert::TryInto;

    #[test]
    fn client_command_convert() {
        assert_eq!(
            Ok(ClientCommand::AddPublication),
            ::aeron_driver_sys::AERON_COMMAND_ADD_PUBLICATION.try_into()
        )
    }
}
