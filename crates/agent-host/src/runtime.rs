use std::io;

use tokio::runtime::{self, Handle};

/// The tokio runtime the MCP server and the agent's session run on, away
/// from the UI thread.
pub struct Runtime(Option<runtime::Runtime>);

impl Runtime {
    pub fn new() -> io::Result<Self> {
        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("levare-agent")
            .enable_all()
            .build()?;
        Ok(Self(Some(runtime)))
    }

    pub fn handle(&self) -> &Handle {
        self.0
            .as_ref()
            .expect("runtime is only taken on drop")
            .handle()
    }
}

impl Drop for Runtime {
    /// Quitting must not wait on a connection or an agent that is still
    /// running: their tasks are dropped where they are.
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}
