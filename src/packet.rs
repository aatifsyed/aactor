#[allow(unused_imports)] // Doc link
use crate::UdpSocket;
#[allow(unused_imports)] // Doc link
use futures::{Sink, Stream};
use std::fmt::{self, Debug, Display, Formatter};
use std::net::SocketAddr;

/// Represents UDP data that is associated with a [`SocketAddr`].
/// - When [`UdpSocket`] is used as a [`Sink`], the `address` represents the destination address.
/// - When [`UdpSocket`] is used as a [`Stream`], the `address` represents the source address.
#[derive(Debug, PartialEq, Eq)]
pub struct AddressedUdp {
    /// The data contained in the UDP. Note that this may be truncated. See module documentation for more.
    pub udp: Vec<u8>,
    /// The associated address.
    pub address: SocketAddr,
}

impl Display for AddressedUdp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Ok(s) = std::str::from_utf8(&self.udp) {
            write!(f, "{}: {} bytes: {:?}", self.address, self.udp.len(), s)
        } else {
            write!(
                f,
                "{}: {} bytes: {:?}",
                self.address,
                self.udp.len(),
                self.udp
            )
        }
    }
}
