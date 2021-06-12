#![allow(unused_imports, dead_code)]
use async_io::Async;
use async_std::io;
use async_std::net::{ToSocketAddrs, UdpSocket};
use futures::stream::{Stream, StreamExt};
use std::borrow::{Borrow, BorrowMut};
use std::future::Future;
use std::net::UdpSocket as UdpSocketSync;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use thiserror::Error;
pub mod stream;

struct UdpSocketStream<'s> {
    socket: UdpSocket,
    buffer: [u8; UdpSocketStream::BUFSIZE],
    current_packet: &'s [u8],
}
impl<'s> UdpSocketStream<'s> {
    const BUFSIZE: usize = 65507;
}
#[cfg(test)]
mod tests {
    use super::*;
    use async_std::test;

    #[test]
    async fn receive_udp() {
        let listener = UdpSocket::bind("localhost:0").await.unwrap();
        let sender = UdpSocket::bind("localhost:0").await.unwrap();
        let sent = b"hello";
        sender
            .send_to(sent, listener.local_addr().unwrap())
            .await
            .unwrap();
        let mut buf = Box::new([0u8; 65536]);
        let (read, from) = listener.recv_from(&mut *buf).await.unwrap();
        assert!(from == sender.local_addr().unwrap());
        assert!(&(*buf)[..read] == sent);
    }

    #[test]
    async fn recieve_two_udp_packets() {
        let sent = b"hello";
        let mut buf = Box::new([0u8; 65536]);
        let listener = UdpSocket::bind("localhost:0").await.unwrap();
        let sender = UdpSocket::bind("localhost:0").await.unwrap();

        sender
            .send_to(sent, listener.local_addr().unwrap())
            .await
            .unwrap();
        sender
            .send_to(sent, listener.local_addr().unwrap())
            .await
            .unwrap();

        let (read, from) = listener.recv_from(&mut *buf).await.unwrap();
        assert!(from == sender.local_addr().unwrap());
        assert!(&(*buf)[..read] == sent);

        let (read, from) = listener.recv_from(&mut *buf).await.unwrap();
        assert!(from == sender.local_addr().unwrap());
        assert!(&(*buf)[..read] == sent);
    }

    #[test]
    async fn discover_max_udp_size_lower() {
        let sent = Box::new([0u8; 65507]);
        let sender = UdpSocket::bind("localhost:0").await.unwrap();
        sender.send_to(&*sent, "localhost:1").await.unwrap();
    }

    #[test]
    #[should_panic]
    async fn discover_max_udp_size_upper() {
        let sent = Box::new([0u8; 65508]);
        let sender = UdpSocket::bind("localhost:0").await.unwrap();
        sender.send_to(&*sent, "localhost:1").await.unwrap();
    }
}
