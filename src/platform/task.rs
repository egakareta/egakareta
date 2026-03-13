use std::future::Future;

#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_background<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_background<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || {
        pollster::block_on(future);
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::spawn_background;
    use web_time::Duration;

    #[test]
    fn executes_background_future() {
        let (sender, receiver) = std::sync::mpsc::channel();
        spawn_background(async move {
            let _ = sender.send(42u8);
        });
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).ok(), Some(42));
    }
}
