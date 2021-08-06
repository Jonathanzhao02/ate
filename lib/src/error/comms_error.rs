use error_chain::error_chain;
use rmp_serde::decode::Error as RmpDecodeError;
use serde_json::Error as JsonError;
use tokio::sync::mpsc as mpsc;
#[cfg(feature="enable_web")]
use ws_stream_wasm::WsErr;

error_chain! {
    types {
        CommsError, CommsErrorKind, ResultExt, Result;
    }
    links {
        SerializationError(super::SerializationError, super::SerializationErrorKind);
        ValidationError(super::ValidationError, super::ValidationErrorKind);
        LoadError(super::LoadError, super::LoadErrorKind);
    }
    foreign_links {
        IO(::tokio::io::Error);
        JoinError(::tokio::task::JoinError);
        UrlError(::url::ParseError);
    }
    errors {
        SendError(err: String) {
            description("sending error while processing communication"),
            display("sending error while processing communication - {}", err),
        }
        ReceiveError(err: String) {
            description("receiving error while processing communication"),
            display("receiving error while processing communication - {}", err),
        }
        NoReplyChannel {
            description("message has no reply channel attached to it")
            display("message has no reply channel attached to it")
        }
        Disconnected {
            description("channel has been disconnected")
            display("channel has been disconnected")
        }
        Timeout {
            description("io timeout")
            display("io timeout")
        }
        NoAddress {
            description("no address to connect to")
            display("no address to connect to")
        }
        Refused {
            description("connection was refused by the destination address")
            display("connection was refused by the destination address")
        }
        ShouldBlock {
            description("operation should have blocked but it didnt")
            display("operation should have blocked but it didnt")
        }
        InvalidDomainName {
            description("the supplied domain name is not valid")
            display("the supplied domain name is not valid")
        }
        RequredExplicitNodeId {
            description("ate is unable to determine the node_id of this root and thus you must explicily specify it in cfg")
            display("ate is unable to determine the node_id of this root and thus you must explicily specify it in cfg")
        }
        ListenAddressInvalid(addr: String) {
            description("could not listen on the address as it is not a valid IPv4/IPv6 address"),
            display("could not listen on the address ({}) as it is not a valid IPv4/IPv6 address", addr),
        }
        RootServerError(err: String) {
            description("error at the root server while processing communication"),
            display("error at the root server while processing communication - {}", err),
        }
        InternalError(err: String) {
            description("internal comms error"),
            display("internal comms error - {}", err),
        }
        #[cfg(feature="enable_ws")]
        #[cfg(feature="enable_web")]
        WebSocketError(err: String) {
            description("web socket error"),
            display("web socket error - {}", err),
        }
        #[cfg(feature="enable_ws")]
        #[cfg(not(feature="enable_web"))]
        WebSocketError(err: String) {
            description("web socket error"),
            display("web socket error - {}", err),
        }
        #[cfg(feature="enable_ws")]
        WebSocketInternalError(err: String) {
            description("web socket internal error"),
            display("web socket internal error - {}", err),
        }
        UnsupportedProtocolError(proto: String) {
            description("unsupported wire protocol"),
            display("unsupported wire protocol ({})", proto),
        }
    }
}

impl From<tokio::time::error::Elapsed>
for CommsError
{
    fn from(_err: tokio::time::error::Elapsed) -> CommsError {
        CommsErrorKind::IO(std::io::Error::new(std::io::ErrorKind::TimedOut, format!("Timeout while waiting for communication channel").to_string())).into()
    }   
}

impl<T> From<mpsc::error::SendError<T>>
for CommsError
{
    fn from(err: mpsc::error::SendError<T>) -> CommsError {
        CommsErrorKind::SendError(err.to_string()).into()
    }   
}

#[cfg(feature="enable_ws")]
#[cfg(feature="enable_web")]
impl From<WsErr>
for CommsError
{
    fn from(err: WsErr) -> CommsError {
        CommsErrorKind::WebSocketError(er.to_string()).into()
    }   
}

#[cfg(feature="enable_ws")]
#[cfg(not(feature="enable_web"))]
impl From<tokio_tungstenite::tungstenite::Error>
for CommsError
{
    fn from(err: tokio_tungstenite::tungstenite::Error) -> CommsError {
        CommsErrorKind::WebSocketError(err.to_string()).into()
    }   
}

#[cfg(feature="enable_tcp")]
#[cfg(feature="enable_ws")]
impl From<tokio_tungstenite::tungstenite::http::uri::InvalidUri>
for CommsError
{
    fn from(err: tokio_tungstenite::tungstenite::http::uri::InvalidUri) -> CommsError {
        CommsErrorKind::WebSocketInternalError(format!("Failed to establish websocket due to an invalid URI - {}", err.to_string())).into()
    }   
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>>
for CommsError
{
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> CommsError {
        CommsErrorKind::SendError(err.to_string()).into()
    }   
}

impl From<tokio::sync::broadcast::error::RecvError>
for CommsError
{
    fn from(err: tokio::sync::broadcast::error::RecvError) -> CommsError {
        CommsErrorKind::ReceiveError(err.to_string()).into()
    }   
}

impl From<super::CommitError>
for CommsError
{
    fn from(err: super::CommitError) -> CommsError {
        match err {
            super::CommitError(super::CommitErrorKind::ValidationError(errs), _) => CommsErrorKind::ValidationError(errs).into(),
            err => CommsErrorKind::InternalError(format!("commit-failed - {}", err.to_string())).into(),
        }
    }   
}

impl From<super::ChainCreationError>
for CommsError
{
    fn from(err: super::ChainCreationError) -> CommsError {
        CommsErrorKind::RootServerError(err.to_string()).into()
    }   
}

impl From<super::ChainCreationErrorKind>
for CommsError
{
    fn from(err: super::ChainCreationErrorKind) -> CommsError {
        CommsErrorKind::RootServerError(err.to_string()).into()
    }   
}

impl From<bincode::Error>
for CommsError
{
    fn from(err: bincode::Error) -> CommsError {
        CommsErrorKind::SerializationError(super::SerializationErrorKind::BincodeError(err).into()).into()
    }   
}

impl From<RmpDecodeError>
for CommsError {
    fn from(err: RmpDecodeError) -> CommsError {
        CommsErrorKind::SerializationError(super::SerializationErrorKind::DecodeError(err).into()).into()
    }
}

impl From<JsonError>
for CommsError {
    fn from(err: JsonError) -> CommsError {
        CommsErrorKind::SerializationError(super::SerializationErrorKind::JsonError(err).into()).into()
    }
}