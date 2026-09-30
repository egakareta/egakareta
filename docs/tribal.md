# Tribal Knowledge

Document any archaeological knowledge for future reference.

## Web resource loading

Contrary to expectations, moving built-in level data out of `.wasm` does **not** reduce wasm-bindgen time.

## AudioWorklet imports

CPAL imports the application's wasm-bindgen JS glue into the AudioWorklet realm.
AudioWorklets and Rayon workers do not inherit the page's import map, so a bare
`@zenfs/core` import in that glue prevents worklet startup even though the game
loads and audio decoding succeeds. Simply resolving that import to `zenfs.js`
is not enough: the vendored loader uses top-level await, dynamic imports, and
browser decompression APIs unavailable in AudioWorklets. The page exposes its
loaded ZenFS namespace, and the web build binds the glue to it without a static
filesystem import. Audio/compute realms leave those unused bindings empty and
must not perform filesystem I/O. Keep this step when changing the build or
storage layer.
