use crate::interface::{ControlError, ControlRequest, Incoming, Outgoing};
use async_std::net::SocketAddrV4;
use async_std::sync::RwLock;
use bidirectional_channel::{ReceivedRequest, Requester, Respond, Responder};
use bimap::BiHashMap;
use futures::select;
use futures::StreamExt;
use thiserror::Error;

struct Forwarder {
    table: RwLock<BiHashMap<Incoming, Outgoing>>,
    control_channel: RwLock<Responder<ReceivedRequest<ControlRequest, Result<(), ControlError>>>>,
}

impl Forwarder {
    async fn control(&self) {
        while let Some(ReceivedRequest {
            request,
            unresponded,
        }) = self.control_channel.write().await.next().await
        {
            let response = match request {
                ControlRequest::Create(incoming, outgoing) => {
                    let mut table = self.table.write().await;
                    table
                        .insert_no_overwrite(incoming, outgoing)
                        .map_err(|(incoming, outgoing)| {
                            match (table.get_by_right(&outgoing), table.get_by_left(&incoming)) {
                                (Some(_), None) => {
                                    ControlError::DuplicateOutgoing(incoming, outgoing)
                                }
                                (None, Some(_)) => {
                                    ControlError::DuplicateIncoming(incoming, outgoing)
                                }
                                (Some(_), Some(_)) => {
                                    ControlError::DuplicateFlow(incoming, outgoing)
                                }
                                (None, None) => panic!("Inconsistent forwarding table"),
                            }
                        })
                }
                _ => todo!(),
            };
            unresponded
                .respond(response)
                .expect("Controller closed unexpectedly");
        }
    }
    async fn forward(&self) {
        loop {
            select! {}
        }
    }
}
