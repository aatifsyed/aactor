use std::fmt::{self, Debug, Display, Formatter};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_io::Async;
use async_std::net::ToSocketAddrs;
use derive_more::AsRef;
use futures::Stream;
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

impl Stream for UdpSocket {
    type Item = io::Result<ReceivedUdp>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pinned = &mut *self;
        match pinned.watcher.poll_readable(cx) {
            Poll::Ready(_ready) => {
                let sync = pinned.watcher.as_ref();
                match sync.recv_from(&mut pinned.buffer) {
                    Ok((len, addr)) => Poll::Ready(Some(Ok(ReceivedUdp {
                        packet: pinned.buffer[..len].to_vec(),
                        source: addr,
                    }))),
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReceivedUdp {
    pub packet: Vec<u8>,
    pub source: SocketAddr,
}

impl Display for ReceivedUdp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Ok(s) = std::str::from_utf8(&self.packet) {
            write!(
                f,
                "{} bytes from {}: {:?}",
                self.packet.len(),
                self.source,
                s
            )
        } else {
            write!(
                f,
                "{} bytes from {}: {:?}",
                self.packet.len(),
                self.source,
                self.packet
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;
    use futures::{stream, StreamExt};
    use itertools::all;

    #[test]
    async fn build_socket() {
        let sock = UdpSocketBuilder::default().build().await.unwrap();
        println!("{:?}", sock);
        println!("{:?}", sock.as_ref().local_addr());
    }

    #[test]
    async fn stream() {
        let test_data = vec!["hello", "goodbye"];
        let receiver = UdpSocketBuilder::default().build().await.unwrap();
        let receiver_address = receiver.as_ref().local_addr().unwrap();

        let sender = &async_std::net::UdpSocket::bind("localhost:0")
            .await
            .unwrap();
        let sender_address = sender.local_addr().unwrap();

        let send = stream::iter(test_data).then(|data| async move {
            let packet = data.as_bytes();
            sender.send_to(packet, receiver_address).await.unwrap();
            let expected = ReceivedUdp {
                packet: packet.to_vec(),
                source: sender_address,
            };
            println!("Sent {}", data);
            expected
        });

        let receive = receiver.map(|r| {
            let packet = r.unwrap();
            println!("{}", packet);
            packet
        });

        let stream = send.zip(receive).collect::<Vec<_>>().await;
        assert!(all(stream, |(expected, actual)| expected == actual));
    }
}

mod socket_builder {
    use super::UdpSocket;
    use async_io::Async;
    use async_std::{io, net::ToSocketAddrs};
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
        CouldntBind(#[from] io::Error),
    }

    pub struct UdpSocketBuilder<A: ToSocketAddrs> {
        pub(crate) address: Option<A>,
        pub(crate) buffer: Option<Vec<u8>>,
    }

    impl Default for UdpSocketBuilder<&str> {
        fn default() -> Self {
            Self {
                address: Some("localhost:0"),
                buffer: Some(vec![0; 65536]),
            }
        }
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
                // lifted from async_std
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
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "could not resolve to any addresses",
                        )
                    })
                    .into())
            }
        }
    }
}
