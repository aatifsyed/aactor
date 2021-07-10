use std::collections::VecDeque;

use crate::{AddressedUdp, UdpSocket};
use async_io::Async;
use async_std::{io, net::ToSocketAddrs};
use thiserror::Error;

/// Errors that could arise when calling `build`
#[derive(Debug, Error)]
pub enum CreateSocketError {
    /// Couldn't convert the given socket address to a real one through the OS
    #[error("Couldn't convert socket address")]
    BadAddress(std::io::Error),
    /// Couldn't bind to the given socket
    #[error("Couldn't bind to socket")]
    CouldntBind(#[from] io::Error),
}

/// Builder
pub struct UdpSocketBuilder<A: ToSocketAddrs> {
    pub(crate) address: A,
    pub(crate) buffer: Vec<u8>,
    pub(crate) outbound: VecDeque<AddressedUdp>,
}

/// Defaults to the following:
/// - An address at `localhost:0`, which will pick up a free port from the OS
/// - A buffer size of 65536 bytes.
/// - An empty outbound queue
impl Default for UdpSocketBuilder<&str> {
    fn default() -> Self {
        Self {
            address: "localhost:0",
            buffer: vec![0; 65536],
            outbound: VecDeque::new(),
        }
    }
}

impl<A: ToSocketAddrs> UdpSocketBuilder<A> {
    /// The address to bind to for this socket
    pub fn address(mut self, addrs: A) -> Self {
        self.address = addrs;
        self
    }
    /// Maximum UDP packet size for this socket. Additional bytes will be discarded
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer = vec![0; size];
        self
    }
    /// A pre-loaded queue of UDP packets to send out from this socket (when used as a sink)
    pub fn outbound<I: IntoIterator<Item = AddressedUdp>>(mut self, outbound: I) -> Self {
        self.outbound = outbound.into_iter().collect();
        self
    }
    /// Attempt to create the [`UdpSocket`]
    pub async fn build(self) -> Result<UdpSocket, CreateSocketError> {
        // lifted from async_std
        let mut last_err = None;

        for addr in self
            .address
            .to_socket_addrs()
            .await
            .map_err(|e| CreateSocketError::BadAddress(e))?
        {
            match Async::<std::net::UdpSocket>::bind(addr) {
                Ok(socket) => {
                    return Ok(UdpSocket {
                        watcher: socket,
                        buffer: self.buffer,
                        outbound: self.outbound,
                    });
                }
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err
            .unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "could not resolve to any addresses",
                )
            })
            .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;

    /// Test that the Socket Builder works
    #[test]
    async fn build_socket() {
        let sock = UdpSocketBuilder::default().build().await.unwrap();
        println!("{:?}", sock);
        println!("{:?}", sock.as_ref().local_addr());
    }
}
