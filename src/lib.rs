use async_trait::async_trait;
use bidirectional_channel::{IncomingRequest, Receiver, Sender};
use futures::StreamExt;
use std::fmt::Debug;

#[async_trait]
pub trait Actor {
    type Request;
    type Response: Debug;
    async fn respond(&mut self, request: Self::Request) -> Self::Response;
}
struct AActor<I: Actor> {
    inner: I,
    channel: Receiver<IncomingRequest<I::Request, I::Response>>,
}

impl<I: Actor> AActor<I> {
    async fn main(&mut self) {
        while let Some(incoming_request) = self.channel.next().await {
            let IncomingRequest(request, responder) = incoming_request;
            let response = self.inner.respond(request).await;
            responder.send(response).unwrap();
        }
    }
}

pub fn run<'s, I: 's + Actor>(
    actor: I,
    capacity: usize,
) -> (
    Sender<<I as Actor>::Request, <I as Actor>::Response>,
    impl 's + futures::Future,
) {
    let (sender, receiver) = bidirectional_channel::bounded(capacity);
    let aactor = Box::new(AActor {
        inner: actor,
        channel: receiver,
    });
    let aactor = Box::leak(aactor);
    (sender, aactor.main())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
