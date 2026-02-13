#[cfg(not(target_arch = "wasm32"))]
fn main() {
    line_dash_lib::run_native_app();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
