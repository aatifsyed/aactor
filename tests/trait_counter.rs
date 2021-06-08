use aactor::Actor;
use async_std::test;
use async_trait::async_trait;
use futures::join;
use futures::{stream, StreamExt};
use log::debug;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counter {
    count: AtomicUsize,
}
#[async_trait]
impl Actor for Counter {
    type Request = ();
    type Response = ();
    async fn respond(&mut self, _request: Self::Request) -> Self::Response {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
async fn counter() {
    let counter = Counter {
        count: AtomicUsize::new(0),
    };
    let (sender, actor) = aactor::run(counter, 10);

    let send_task = async {
        let mut stream = stream::repeat(()).take(5);
        while let Some(ping) = stream.next().await {
            let response = sender.send(ping).await.unwrap();
            debug!("sent a ping");
            response.await.unwrap();
        }
        drop(sender) // End the test
    };

    join!(send_task, actor);
}
