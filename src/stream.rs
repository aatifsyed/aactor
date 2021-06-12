use std::fmt::{self, Debug, Formatter};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_io::Async;
use async_std::{io, net::ToSocketAddrs};
use futures::{join, Stream, StreamExt};

pub struct UdpSocket<const N: usize> {
    watcher: Async<std::net::UdpSocket>,
    buffer: Box<[u8; N]>,
}

impl<const N: usize> Stream for UdpSocket<N> {
    type Item = io::Result<(usize, SocketAddr)>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pinned: &mut Self = &mut *self;
        match pinned.watcher.poll_readable(cx) {
            Poll::Ready(_ready) => {
                let buffer = &mut (*pinned.buffer)[..];
                let sync = pinned.watcher.as_ref();
                Poll::Ready(Some(sync.recv_from(buffer)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct UdpPacket {
    pub packet: Vec<u8>,
    pub address: SocketAddr,
}

impl<S> From<(S, SocketAddr)> for UdpPacket
where
    S: Into<Vec<u8>>,
{
    fn from((packet, address): (S, SocketAddr)) -> Self {
        Self {
            packet: packet.into(),
            address,
        }
    }
}

impl Debug for UdpPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut str = &*format!("{:?}", self.packet);
        if let Ok(utf8) = std::str::from_utf8(&self.packet) {
            str = utf8;
        }
        write!(
            f,
            "{} bytes of UDP from {}: {:?}",
            self.packet.len(),
            self.address,
            str // Debug so that we e.g escape newlines
        )
    }
}

impl<const N: usize> UdpSocket<N> {
    // Copied from [`async_std::net::udp::UdpSocket::bind`]
    pub async fn bind<A: ToSocketAddrs>(address: A) -> io::Result<UdpSocket<N>> {
        let mut last_err = None;
        let resolved_addresses = address.to_socket_addrs().await?;
        for address in resolved_addresses {
            match Async::<std::net::UdpSocket>::bind(address) {
                Ok(socket) => {
                    return Ok(UdpSocket {
                        watcher: socket,
                        buffer: Box::new([0; N]),
                    })
                }
                Err(err) => last_err = Some(err),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any addresses",
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;

    #[test]
    async fn stream_udp() {
        let recv_socket = UdpSocket::<65535>::bind("127.0.0.1:0").await.unwrap();
        let recv_address = recv_socket.watcher.as_ref().local_addr().unwrap();
        let send_socket = async_std::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .unwrap();
        let sent = vec!["hello", "my", "name", "is", "Aatif"];
        let sender = async {
            for payload in sent.iter() {
                send_socket
                    .send_to(payload.as_bytes(), recv_address)
                    .await
                    .unwrap();
            }
        };

        recv_socket
            .then(|result| async {
                let (len, address) = result.unwrap();
                let data = &recv_socket.buffer[..len];
                let packet: UdpPacket = (data, address).into();
                println!("{:?}", packet);
                packet
            })
            .take_until(sender)
            .collect::<Vec<_>>();
    }
}
