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
        #[doc = "Add a Publication"]
        AddPublication = AERON_COMMAND_ADD_PUBLICATION,
        #[doc = "Remove a Publication"]
        RemovePublication = AERON_COMMAND_REMOVE_PUBLICATION,
        #[doc = "Add an Exclusive Publication"]
        AddExclusivePublication = AERON_COMMAND_ADD_EXCLUSIVE_PUBLICATION,
        #[doc = "Add a Subscriber"]
        AddSubscription = AERON_COMMAND_ADD_SUBSCRIPTION,
        #[doc = "Remove a Subscriber"]
        RemoveSubscription = AERON_COMMAND_REMOVE_SUBSCRIPTION,
        #[doc = "Keepalaive from Client"]
        ClientKeepalive = AERON_COMMAND_CLIENT_KEEPALIVE,
        #[doc = "Add Destination to an existing Publication"]
        AddDestination = AERON_COMMAND_ADD_DESTINATION,
        #[doc = "Remove Destination from an existing Publication"]
        RemoveDestination = AERON_COMMAND_REMOVE_DESTINATION,
        #[doc = "Add a Counter to the counters manager"]
        AddCounter = AERON_COMMAND_ADD_COUNTER,
        #[doc = "Remove a Counter from the counters manager"]
        RemoveCounter = AERON_COMMAND_REMOVE_COUNTER,
        #[doc = "Close indication from Client"]
        ClientClose = AERON_COMMAND_CLIENT_CLOSE,
        #[doc = "Add Destination for existing Subscription"]
        AddRcvDestination = AERON_COMMAND_ADD_RCV_DESTINATION,
        #[doc = "Remove Destination for existing Subscription"]
        RemoveRcvDestination = AERON_COMMAND_REMOVE_RCV_DESTINATION,
        #[doc = "Request the driver to terminate"]
        TerminateDriver = AERON_COMMAND_TERMINATE_DRIVER,
    }
);

define_enum!(
    #[doc = "Responses from the Media Driver to client commands"]
    pub enum DriverResponse {
        #[doc = "Error Response as a result of attempting to process a client command operation"]
        OnError = AERON_RESPONSE_ON_ERROR,
        #[doc = "Subscribed Image buffers are available notification"]
        OnAvailableImage = AERON_RESPONSE_ON_AVAILABLE_IMAGE,
        #[doc = "New Publication buffers are ready notification"]
        OnPublicationReady = AERON_RESPONSE_ON_PUBLICATION_READY,
        #[doc = "Operation has succeeded"]
        OnOperationSuccess = AERON_RESPONSE_ON_OPERATION_SUCCESS,
        #[doc = "Inform client of timeout and removal of an inactive Image"]
        OnUnavailableImage = AERON_RESPONSE_ON_UNAVAILABLE_IMAGE,
        #[doc = "New Exclusive Publication buffers are ready notification"]
        OnExclusivePublicationReady = AERON_RESPONSE_ON_EXCLUSIVE_PUBLICATION_READY,
        #[doc = "New Subscription is ready notification"]
        OnSubscriptionReady = AERON_RESPONSE_ON_SUBSCRIPTION_READY,
        #[doc = "New counter is ready notification"]
        OnCounterReady = AERON_RESPONSE_ON_COUNTER_READY,
        #[doc = "Inform clients of removal of counter"]
        OnUnavailableCounter = AERON_RESPONSE_ON_UNAVAILABLE_COUNTER,
        #[doc = "Inform clients of client timeout"]
        OnClientTimeout = AERON_RESPONSE_ON_CLIENT_TIMEOUT,
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
