use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{AddressedUdp, UdpSocketBuilder};
use async_io::Async;
use async_std::net::ToSocketAddrs;
use derive_more::AsRef;
use futures::{Sink, Stream};
use tracing::{self, debug, info, instrument, trace, warn};

/// Bound socket with buffer
#[derive(AsRef, Debug)]
pub struct UdpSocket {
    #[as_ref(forward)] // Bugged?
    pub(crate) watcher: Async<std::net::UdpSocket>,
    pub(crate) buffer: Vec<u8>,
    pub(crate) outbound: VecDeque<AddressedUdp>,
}

impl UdpSocket {
    pub fn builder<A: ToSocketAddrs>() -> UdpSocketBuilder<A> {
        UdpSocketBuilder {
            address: None,
            buffer: None,
            outbound: None,
        }
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
        while let Some(item) = self.outbound.pop_front() {
            debug!("Popped {}", item);
            match self.watcher.poll_writable(cx) {
                Poll::Ready(Ok(_)) => {
                    debug!("ready for sending");
                    let sync = self.watcher.as_ref();
                    if let Err(e) = sync.send_to(&item.udp, item.address) {
                        warn!("err on send: {:?}", e);
                        return Poll::Ready(Err(e));
                    };
                }
                err_or_pending => {
                    debug!("Err or pending: {:?}", err_or_pending);
                    return err_or_pending;
                }
            }
        }
        Poll::Ready(Ok(()))
    }

    #[instrument]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;
    use futures::{join, stream, StreamExt};
    use itertools::all;
    use tracing_subscriber;

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

    #[test]
    async fn sink() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .pretty()
            .init();

        let test_data = vec!["hello", "goodbye"];
        let sender = UdpSocketBuilder::default()
            .buffer_size(20)
            .build()
            .await
            .unwrap();

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
            loop {
                let mut buf = Vec::from([0; 1024]);
                let (len, address) = receiver.recv_from(&mut buf).await.unwrap();
                buf.truncate(len);
                info!("Received: {}", AddressedUdp { udp: buf, address })
            }
        };

        join!(sender_task, receiver_task);
    }
}
