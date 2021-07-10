use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::AddressedUdp;
use async_io::Async;
use async_std::net::ToSocketAddrs;
use derive_more::AsRef;
use futures::{Sink, Stream};
use thiserror::Error;
use tracing::{self, debug, instrument};

/// A bound UDP socket, which acts as both a [`Sink`] and a [`Stream`] for [`AddressedUdp`]
#[derive(AsRef)]
pub struct UdpSocket {
    #[as_ref(forward)] // Bugged?
    pub(crate) watcher: Async<std::net::UdpSocket>,
    pub(crate) buffer: Vec<u8>,
    pub(crate) outbound: VecDeque<AddressedUdp>,
}

impl Debug for UdpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("UdpSocket")
            .field("socket", self.watcher.as_ref())
            .field("buffer_length", &self.buffer.len())
            .field("outbound", &self.outbound)
            .finish()
    }
}

impl Stream for UdpSocket {
    type Item = io::Result<AddressedUdp>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pinned = &mut *self;
        match pinned.watcher.poll_readable(cx) {
            Poll::Ready(_ready) => {
                let sync = pinned.watcher.as_ref();
                match sync.recv_from(&mut pinned.buffer) {
                    Ok((len, addr)) => Poll::Ready(Some(Ok(AddressedUdp {
                        udp: pinned.buffer[..len].to_vec(),
                        address: addr,
                    }))),
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Sink<AddressedUdp> for UdpSocket {
    type Error = io::Error;
    #[instrument]
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.watcher.poll_writable(cx)
    }
    #[instrument]
    fn start_send(mut self: Pin<&mut Self>, item: AddressedUdp) -> Result<(), Self::Error> {
        self.outbound.push_back(item);
        Ok(())
    }

    #[instrument]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            match self.watcher.poll_writable(cx) {
                Poll::Ready(Ok(_)) => {
                    debug!("Ready for sending");
                    if let Some(packet) = self.outbound.pop_front() {
                        debug!("More packets to send");
                        match self.watcher.as_ref().send_to(&packet.udp, packet.address) {
                            Ok(_) => {
                                debug!("Sent a packet successfully");
                                continue; // Could be more on the queue
                            }
                            Err(e) => {
                                debug!("Failed to send a packet");
                                return Poll::Ready(Err(e));
                            }
                        }
                    } else {
                        debug!("Outbound queue is empty");
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(e)) => {
                    debug!("Error");
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => {
                    debug!("Pending");
                    return Poll::Pending;
                }
            }
        }
    }

    #[instrument]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

impl UdpSocket {
    /// Bind a socket at `address`, which can receive packets up to `buffer_size` in length
    pub async fn new<A: ToSocketAddrs>(
        address: A,
        buffer_size: usize,
    ) -> Result<Self, CreateSocketError> {
        // lifted from async_std
        let mut last_err = None;

        for addr in address
            .to_socket_addrs()
            .await
            .map_err(|e| CreateSocketError::BadAddress(e))?
        {
            match Async::<std::net::UdpSocket>::bind(addr) {
                Ok(socket) => {
                    return Ok(UdpSocket {
                        watcher: socket,
                        buffer: vec![0; buffer_size],
                        outbound: Default::default(),
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
    /// Creates a socket on a free port at `127.0.0.1`, with a buffer size of 65536
    pub async fn default() -> Result<Self, CreateSocketError> {
        Self::new("127.0.0.1:0", 65536).await
    }
}

/// Errors that could arise when creating the socket
#[derive(Debug, Error)]
pub enum CreateSocketError {
    /// Couldn't convert the given socket address to a real one through the OS
    #[error("Couldn't convert socket address")]
    BadAddress(std::io::Error),
    /// Couldn't bind to the given socket
    #[error("Couldn't bind to socket")]
    CouldntBind(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;
    use futures::{join, stream, StreamExt};
    use itertools::all;
    use tracing::info;

    /// Send UDP from a standard socket, and ensure that our receiving socket can [`Stream`] those received packets
    #[test]
    async fn stream() {
        let test_data = vec!["hello", "goodbye"];
        let receiver = UdpSocket::new("127.0.0.1:0", 65536).await.unwrap();
        let receiver_address = receiver.as_ref().local_addr().unwrap();

        let sender = &async_std::net::UdpSocket::bind("localhost:0")
            .await
            .unwrap();
        let sender_address = sender.local_addr().unwrap();

        let send = stream::iter(test_data).then(|data| async move {
            let packet = data.as_bytes();
            sender.send_to(packet, receiver_address).await.unwrap();
            let expected = AddressedUdp {
                udp: packet.to_vec(),
                address: sender_address,
            };
            info!("Sent {}", data);
            expected
        });

        let receive = receiver.map(|r| {
            let packet = r.unwrap();
            info!(" Received {}", packet);
            packet
        });

        let stream = send.zip(receive).collect::<Vec<_>>().await;
        assert!(all(stream, |(expected, actual)| expected == actual));
    }

    /// [`Sink`] some test data to our socket, and check that a standard UDP socket can receive it
    #[test]
    async fn sink() {
        let test_data = ["hello", "goodbye"];
        let sender = UdpSocket::new("127.0.0.1:0", 65536).await.unwrap();
        let sender_address = sender.watcher.as_ref().local_addr().unwrap();

        let receiver = async_std::net::UdpSocket::bind("localhost:0")
            .await
            .unwrap();
        let receiver_address = receiver.local_addr().unwrap();

        let sender_task = stream::iter(test_data)
            .then(|data| async move {
                info!("Sending {}", data);
                Ok(AddressedUdp {
                    udp: Vec::from(data.as_bytes()),
                    address: receiver_address,
                })
            })
            .forward(sender);

        let receiver_task = async {
            for expected in test_data {
                let mut buf = Vec::from([0; 1024]);
                let (len, address) = receiver.recv_from(&mut buf).await.unwrap();
                buf.truncate(len);
                let addressed_udp = AddressedUdp { udp: buf, address };
                info!("Received: {}", addressed_udp);
                assert_eq!(addressed_udp.address, sender_address);
                assert_eq!(addressed_udp.udp, expected.as_bytes().to_owned());
            }
        };

        let (_, _) = join!(sender_task, receiver_task);
    }
}
