//! A Recipe for reading events from a flume channel.

use beacn_lib::flume::Receiver;
//use futures::Stream;
use iced::advanced::subscription::{EventStream, Hasher, Recipe};
use iced::futures::Stream;
use std::pin::Pin;
use std::sync::Arc;

pub struct TrackedReceiver<T, Out, F>
where
    F: Fn(T) -> Out + Send + Sync + 'static,
    T: Send + 'static,
    Out: Send + 'static,
{
    pub id: &'static str,
    pub rx: Receiver<T>,
    pub map_fn: F,
}

impl<T, Out, F> Recipe for TrackedReceiver<T, Out, F>
where
    F: Fn(T) -> Out + Send + Sync + 'static,
    T: Send + 'static,
    Out: Send + 'static,
{
    type Output = Out;

    // Kept exactly as you had it: &mut Hasher
    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        self.id.hash(state);
    }

    fn stream(self: Box<Self>, _: EventStream) -> Pin<Box<dyn Stream<Item = Self::Output> + Send>> {
        let rx = self.rx;
        let map_fn = Arc::new(self.map_fn);

        let s = iced::futures::stream::unfold((rx, map_fn), move |(rx, map_fn)| async move {
            match rx.recv_async().await {
                Ok(data) => {
                    let current_map = Arc::clone(&map_fn);
                    let mapped_msg = current_map(data);

                    Some((mapped_msg, (rx, map_fn)))
                }
                Err(_) => None,
            }
        });
        Box::pin(s)
    }
}
