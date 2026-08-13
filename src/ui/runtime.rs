use iced_futures::MaybeSend;
use tokio::runtime::Handle;

/// Wraps a Handle to the shared runtime so iced reuses it instead of spinning up its own.
pub struct SharedTokioExecutor(Handle);

impl iced::Executor for SharedTokioExecutor {
    fn new() -> Result<Self, std::io::Error> {
        Ok(Self(Handle::current()))
    }

    fn spawn(&self, future: impl Future<Output = ()> + MaybeSend + 'static) {
        self.0.spawn(future);
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}
