use std::fmt::{self, Debug, Display, Formatter};
use std::net::SocketAddr;

#[derive(Debug, PartialEq, Eq)]
pub struct AddressedUdp {
    pub udp: Vec<u8>,
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
