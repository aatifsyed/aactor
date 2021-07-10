#![deny(missing_docs, unused)]
//! This crate provides a UDP socket which implements [`Sink`] and [`Stream`]

#[allow(unused_imports)] // For docs
use futures::{Sink, Stream};

mod packet;
mod socket;

pub use packet::AddressedUdp;
pub use socket::{CreateSocketError, UdpSocket};
