/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::future::Future;

#[cfg(target_arch = "wasm32")]
pub fn spawn_background<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
mod native_runtime {
    use std::{future::Future, sync::OnceLock};

    use tokio::runtime::{Builder, Handle};

    /// Shared current-thread Tokio runtime driven on a dedicated OS thread.
    ///
    /// Background platform work runs on a shared current-thread runtime so it
    /// can progress independently of the thread that requested it. The runtime
    /// is driven on a dedicated OS thread and tasks are spawned through its
    /// cloned [`Handle`] from any caller thread.
    fn handle() -> &'static Handle {
        static HANDLE: OnceLock<Handle> = OnceLock::new();
        HANDLE.get_or_init(|| {
            let runtime = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build eg-platform background runtime");
            let handle = runtime.handle().clone();
            std::thread::Builder::new()
                .name("eg-platform-background".into())
                .spawn(move || {
                    // Drive the runtime forever; `pending` never resolves, so
                    // the driver thread keeps polling spawned tasks.
                    runtime.block_on(std::future::pending::<()>());
                })
                .expect("failed to spawn eg-platform background driver thread");
            handle
        })
    }

    pub fn spawn<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        drop(handle().spawn(future));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_background<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    native_runtime::spawn(future);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use web_time::Duration;

    use super::spawn_background;

    #[test]
    fn executes_background_future() {
        let (sender, receiver) = std::sync::mpsc::channel();
        spawn_background(async move {
            let _ = sender.send(42u8);
        });
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).ok(), Some(42));
    }
}
