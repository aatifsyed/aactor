use std::fmt::{self, Debug, Display, Formatter};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::builder::UdpSocketBuilder;
use async_io::Async;
use async_std::net::ToSocketAddrs;
use derive_more::AsRef;
use futures::Stream;

/// Bound socket with buffer
#[derive(AsRef, Debug)]
pub struct UdpSocket {
    #[as_ref(forward)] // Bugged?
    pub(crate) watcher: Async<std::net::UdpSocket>,
    pub(crate) buffer: Vec<u8>,
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
