use crate::UdpSocket;
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
