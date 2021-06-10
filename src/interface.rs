use async_std::net::SocketAddrV4;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Incoming(SocketAddrV4);
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Outgoing(SocketAddrV4);

pub enum ControlRequest {
    Create(Incoming, Outgoing),
    Delete(Incoming, Outgoing),
}

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("{0:?} already configured in flow {0:?} -> {1:?}")]
    DuplicateIncoming(Incoming, Outgoing),
    #[error("{1:?} already configured in flow {0:?} -> {1:?}")]
    DuplicateOutgoing(Incoming, Outgoing),
    #[error("Flow {0:?} -> {1:?} is already programmed")]
    DuplicateFlow(Incoming, Outgoing),
}
