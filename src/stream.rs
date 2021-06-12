use std::fmt::{self, Debug, Formatter};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_io::Async;
use async_std::{io as aio, net::ToSocketAddrs};
use derive_more::{AsMut, AsRef, Deref, DerefMut};
use futures::{join, Stream, StreamExt};
pub use socket_builder::UdpSocketBuilder;

/// Bound socket with buffer
#[derive(AsRef, Debug)]
pub struct UdpSocket {
    #[as_ref(forward)] // Bugged?
    watcher: Async<std::net::UdpSocket>,
    buffer: Vec<u8>,
}

impl UdpSocket {
    pub fn builder<A: ToSocketAddrs>() -> UdpSocketBuilder<A> {
        UdpSocketBuilder {
            address: None,
            buffer: None,
        }
    }
}

impl<'u> Stream for UdpSocket {
    type Item = ReceivedUdp<'u>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }
}

pub struct ReceivedUdp<'s> {
    pub packet: &'s [u8],
    pub source: SocketAddr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;

    #[test]
    async fn build_socket() {
        let sock = UdpSocket::builder()
            .address("localhost:0")
            .buffer_size(10)
            .build()
            .await
            .unwrap();
        println!("{:?}", sock);
        println!("{:?}", sock.as_ref().local_addr());
    }
}

mod socket_builder {
    use super::UdpSocket;
    use async_io::Async;
    use async_std::{io as aio, net::ToSocketAddrs};
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum CreateSocketError {
        #[error("Must provide an address")]
        NoAddress,
        #[error("Must provide a buffer size")]
        NoBufferSize,
        #[error("Couldn't convert to socket addresses")]
        BadAddress(std::io::Error),
        #[error("Couldn't bind to socket")]
        CouldntBind(#[from] aio::Error),
    }

    pub struct UdpSocketBuilder<A: ToSocketAddrs> {
        pub(crate) address: Option<A>,
        pub(crate) buffer: Option<Vec<u8>>,
    }

    impl<A: ToSocketAddrs> UdpSocketBuilder<A> {
        pub fn address(mut self, addrs: A) -> Self {
            self.address = Some(addrs);
            self
        }
        pub fn buffer_size(mut self, size: usize) -> Self {
            self.buffer = Some(vec![0; size]);
            self
        }
        pub async fn build(self) -> Result<UdpSocket, CreateSocketError> {
            if self.address.is_none() {
                return Err(CreateSocketError::NoAddress);
            } else if self.buffer.is_none() {
                return Err(CreateSocketError::NoBufferSize);
            } else {
                let mut last_err = None;

                for addr in self
                    .address
                    .unwrap()
                    .to_socket_addrs()
                    .await
                    .map_err(|e| CreateSocketError::BadAddress(e))?
                {
                    match Async::<std::net::UdpSocket>::bind(addr) {
                        Ok(socket) => {
                            return Ok(UdpSocket {
                                watcher: socket,
                                buffer: self.buffer.unwrap(),
                            });
                        }
                        Err(err) => last_err = Some(err),
                    }
                }

                Err(last_err
                    .unwrap_or_else(|| {
                        aio::Error::new(
                            aio::ErrorKind::InvalidInput,
                            "could not resolve to any addresses",
                        )
                    })
                    .into())
            }
        }
    }
}
