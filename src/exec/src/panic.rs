use std::panic::AssertUnwindSafe;

use futures::FutureExt;

// hook will be called when task panicked
pub fn spawn_with_hook<R, H>(
    name: String,
    fut: impl Future<Output = R> + Send + 'static,
    panic_hook: H,
) -> tokio::task::JoinHandle<R>
where
    R: Send + 'static,
    H: FnOnce(String) -> R + Send + Sync + 'static,
{
    let wrapper = AssertUnwindSafe(fut).catch_unwind().map(move |ret| match ret {
        Ok(inner) => inner,
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            let panic_msg = format!("[{name}] panicked: {msg}");
            tracing::error!("{panic_msg}");
            panic_hook(panic_msg)
        }
    });

    tokio::spawn(wrapper)
}
