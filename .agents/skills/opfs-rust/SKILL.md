---
name: opfs-rust
description: Use the `opfs` Rust crate to write file system code that runs identically on native (via tokio::fs) and web (via the browser's OPFS API). Load when writing, reviewing, or debugging code that uses opfs, when the user asks how to read/write/manage files in a cross-platform Rust app, when integrating with the Origin Private File System, or when choosing between the persistent, native, web, and memory backends. Covers app_specific_dir, DirectoryHandle/FileHandle/WritableFileStream, the three required trait imports, range reads, WriteParams advanced writes, and platform-specific gotchas.
---

# Using the `opfs` Rust Crate

The `opfs` crate exposes a small, opinionated file-system API that resolves to the right backend at compile time:

| Backend            | Active when...                     | Underlying API                               |
| ------------------ | ---------------------------------- | -------------------------------------------- |
| `opfs::persistent` | Always available (re-export)       | Picks native or web per target               |
| `opfs::native`     | `cfg(not(target_arch = "wasm32"))` | `tokio::fs`                                  |
| `opfs::web`        | `cfg(target_arch = "wasm32")`      | Browser OPFS via `web-sys`                   |
| `opfs::memory`     | Always available                   | In-memory `HashMap` + `Rc<RefCell<Vec<u8>>>` |

Write code against `opfs::persistent` unless you have a specific reason to pin to a backend (e.g. tests should use `opfs::memory`).

---

## Quick Start

The same code runs on native and web:

```rust
use opfs::persistent::{DirectoryHandle, FileHandle, WritableFileStream, app_specific_dir};
use opfs::persistent;
use opfs::{CreateWritableOptions, GetFileHandleOptions};

// Trait imports are required to call methods on the handles.
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};

async fn write_and_read(dir: DirectoryHandle) -> persistent::Result<()> {
    let file = dir
        .get_file_handle_with_options(
            "hello.txt",
            &GetFileHandleOptions { create: true },
        )
        .await?;

    let mut writer = file
        .create_writable_with_options(&CreateWritableOptions {
            keep_existing_data: false,
        })
        .await?;

    writer.write_at_cursor_pos(b"Hello, OPFS!").await?;
    writer.close().await?;

    let bytes = file.read().await?;
    println!("{} bytes: {:?}", bytes.len(), String::from_utf8_lossy(&bytes));
    Ok(())
}

#[tokio::main]
async fn main() -> persistent::Result<()> {
    let dir = app_specific_dir().await?;
    write_and_read(dir).await
}
```

`app_specific_dir()` returns the OPFS root on web, and the OS user data dir on native (e.g. `~/.local/share` on Linux, resolved through the `dirs` crate).

---

## Core Concepts

### The three handle types

```text
DirectoryHandle  ──get_file_handle_with_options──▶  FileHandle
       │                                              │
       │                                              ├── read / read_range / size
       │                                              │
       │                                              └── create_writable_with_options ──▶ WritableFileStream
       │                                                                                          │
       └── get_directory_handle_with_options (nested)                                            ├── write_at_cursor_pos
       │                                                                                          ├── write_with_params (Write/Seek/Truncate)
       │                                                                                          ├── seek / truncate
       │                                                                                          └── close  (REQUIRED to flush)
       │
       ├── remove_entry / remove_entry_with_options
       └── entries()  (returns a futures::Stream of (name, DirectoryEntry))
```

All three are `private::Sealed` — you cannot implement them yourself. The traits `DirectoryHandle`, `FileHandle`, `WritableFileStream` are the public extension points.

### Trait imports are mandatory

Without the trait `use ... as _;` lines, methods like `get_file_handle_with_options` will not resolve. This is the #1 source of "method not found" errors:

```rust
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};
```

If you only use one of them, import only that one.

### Options structs (all field-named, no defaults)

| Type                        | Fields                                     | Meaning                                                          |
| --------------------------- | ------------------------------------------ | ---------------------------------------------------------------- |
| `GetFileHandleOptions`      | `create: bool`                             | `true` creates the file if missing                               |
| `GetDirectoryHandleOptions` | `create: bool`                             | `true` creates the directory if missing                          |
| `CreateWritableOptions`     | `keep_existing_data: bool`                 | `false` truncates the file before opening                        |
| `FileSystemRemoveOptions`   | `recursive: bool`                          | only matters for directories; required to remove a non-empty dir |
| `WriteParams`               | `command_type`, `data`, `position`, `size` | advanced write/seek/truncate in one call                         |
| `WriteCommandType`          | `Write` \| `Seek` \| `Truncate`            | discriminant for `WriteParams`                                   |

There is no `Default`; construct every options struct explicitly with `..Default::not_implemented()` style. This is intentional.

### The cursor model

`WritableFileStream` carries an internal cursor. `write_at_cursor_pos` writes at the cursor and advances it. `keep_existing_data: true` does **not** preserve the cursor — the cursor always starts at `0` when you call `create_writable_with_options`. To write at an arbitrary offset use `WriteParams` with `command_type: Write` + `position`, or call `seek` first.

`close()` is required to flush the stream. Skipping it is the second most common bug.

### Errors are platform-specific

- `opfs::persistent::Error` is the error of whichever backend compiled in (`std::io::Error` on native, `JsValue` on wasm32, `String` on memory).
- `opfs::persistent::Result<T>` is the matching alias.
- Code that wants to match on the inner error must feature-gate or use `Display`/`Debug`.

### `entries()` is a stream, not a Vec

`dir.entries()` returns `impl Stream<Item = Result<(String, DirectoryEntry<...>), Error>>`. You need `futures::StreamExt` to `.collect()` or iterate it. The stream resolves immediately (it is `futures::stream::iter` of pre-fetched entries); it is **not** a live directory watcher.

---

## Common Patterns

### Range-based reads (memory and native; web uses `Blob.slice`)

```rust
let bytes = file.read_range(0..5).await?;        // exclusive end
let bytes = file.read_range(..=4).await?;       // inclusive end
let bytes = file.read_range(7..).await?;        // from offset
let bytes = file.read_range(..).await?;         // whole file
let bytes = file.read_range(100..).await?;      // past EOF → empty Vec
```

All three backends normalize ranges the same way (clamp end to file size, return empty when start ≥ size).

### Append-style writes without truncating

```rust
// 1) Open with keep_existing_data: true (does NOT preserve cursor — starts at 0)
let mut writer = file
    .create_writable_with_options(&CreateWritableOptions { keep_existing_data: true })
    .await?;

// 2) Seek to end, then write
let end = file.size().await?;
writer.seek(end).await?;
writer.write_at_cursor_pos(new_bytes).await?;
writer.close().await?;
```

### Random-access write at a known offset

Use `write_with_params` with `command_type: Write` and an explicit `position`. This writes at the offset **without** moving the cursor (matches the Web OPFS `WriteCommandType.write` semantics):

```rust
use opfs::{WriteCommandType, WriteParams};

writer
    .write_with_params(&WriteParams {
        command_type: WriteCommandType::Write,
        data: Some(b"patched".to_vec()),
        position: Some(10),
        size: None,
    })
    .await?;
```

### Truncate via WriteParams

```rust
writer
    .write_with_params(&WriteParams {
        command_type: WriteCommandType::Truncate,
        data: None,
        position: None,
        size: Some(20),
    })
    .await?;
```

### Iterate a directory

```rust
use futures::StreamExt;

let mut stream = dir.entries().await?;
while let Some(item) = stream.next().await {
    let (name, entry) = item?;
    match entry {
        opfs::DirectoryEntry::File(_)     => println!("file: {name}"),
        opfs::DirectoryEntry::Directory(_) => println!("dir:  {name}"),
    }
}
```

### Recursive remove

```rust
use opfs::FileSystemRemoveOptions;

dir.remove_entry_with_options("nested_dir", &FileSystemRemoveOptions { recursive: true })
    .await?;
```

`remove_entry` (no options) on a non-empty directory will fail on the memory backend and on the native backend's `remove_dir` (it does succeed on tokio's `remove_dir_all` only via the explicit `recursive: true` path).

### Use the memory backend in tests

```rust
use opfs::memory::DirectoryHandle;

let dir = DirectoryHandle::default();
let mut file = opfs::DirectoryHandle::get_file_handle_with_options(
    &dir, "x.bin", &opfs::GetFileHandleOptions { create: true },
).await.unwrap();
// ... same trait API as persistent
```

The memory backend's `WritableFileStream::seek` returns an error if you try to seek past the end — useful for catching off-by-one bugs that would silently succeed on native/web.

---

## Gotchas

1. **Forgetting the trait imports.** The compiler error names the trait but not that it is in the crate root. Fix: `use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};` at the top of every module that calls these methods.

2. **Forgetting `close()`.** `WritableFileStream` buffers. On native this means the data may not be visible to a subsequent `read()` or to another process; on web it means the `File` blob may not be finalized. Always `await writer.close()` before relying on the bytes.

3. **`keep_existing_data: true` does not keep the cursor.** It only controls whether the file is truncated at open time. The cursor still starts at `0`. To append, `seek(file.size())` first.

4. **`get_file_handle_with_options(name, &GetFileHandleOptions { create: false })` errors on missing files.** There is no "open or fail quietly" variant. On native the error is `std::io::ErrorKind::NotFound`; on memory the error is the string `"'<name>' does not exist"`; on web it is a `JsValue` DOMException. Match via `Display`/`Debug` or feature-gate.

5. **No `Default` for any options struct.** `GetFileHandleOptions::default()` does not exist. Build the struct literally each time.

6. **`entries()` is a one-shot snapshot.** It is implemented as `futures::stream::iter` of entries already read from disk. It does not watch the directory; if you need to observe new files, re-call `entries()`.

7. **Native `remove_entry` without `recursive: true` refuses non-empty directories** on the memory backend (returns `"Directory 'X' is not empty"`). Native falls through to `tokio::fs::remove_dir` which only succeeds on empty dirs. Always pass `FileSystemRemoveOptions { recursive: true }` when you do not know the contents.

8. **The `Error` type changes with the backend.** Code that wants to `match` on the error must either feature-gate on `target_arch = "wasm32"` or use `.to_string()` / `Debug`. `opfs::persistent::Error` is the right type to put in function signatures for portable code; the inner type is hidden.

9. **`FileHandle` and `DirectoryHandle` are `Clone` on all backends**, but cloning does **not** give you an independent OS handle — it is a cheap clone of the path/JS-handle wrapper. Closing one does not close the other. There is no `Drop` that flushes; you must `close()` the `WritableFileStream` explicitly.

10. **`read_range` with a start ≥ file size returns an empty `Vec`, not an error.** Use this to safely probe offsets without pre-checking the size.

11. **Web target needs a browser context.** The `web` module references `web_sys::window().navigator().storage()`, which only exists in a browser. Running wasm32 unit tests under `wasm-bindgen-test` requires a headless browser (chromium/firefox via the test runner). Pure native unit tests on the same crate work via `cargo test` against the `native` backend.

12. **The `tokio` dep is gated to non-wasm targets** (`cfg(not(target_arch = "wasm32"))`). On wasm32 builds you should not depend on tokio unless your own code requires it; the `opfs` API is plain `async fn` and works on any executor.

13. **`FileHandle::get_file()` exists only on the web backend** and is an inherent method (not on the trait). It is a browser-only escape hatch to get a `web_sys::File`; do not use it in portable code.

---

## Validation Checklist

Before shipping code that uses `opfs`, verify each item:

- [ ] **Trait imports present** — every module that touches handles has `use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};` (or the subset it uses).
- [ ] **All handles are `await`-ed** — `get_file_handle_with_options`, `create_writable_with_options`, `write_*`, `close`, `read`, `read_range`, `size`, `entries`, `remove_entry*` are all `async`. Missing `.await` is a type error but easy to miss in `?`-chained expressions.
- [ ] **Every `WritableFileStream` is `close()`-d** before the bytes are assumed to be persisted.
- [ ] **`create_writable_with_options` is called on `&mut FileHandle`** (the trait method takes `&mut self`). Re-binding the stream requires re-opening.
- [ ] **Errors are handled** — the function returns `opfs::persistent::Result<_>` (or the appropriate per-backend alias) and uses `?` rather than `.unwrap()` in non-test code.
- [ ] **Range reads clamp gracefully** — if you pass user-supplied offsets, confirm `read_range` returns empty for out-of-range, not an error.
- [ ] **Tests use the memory backend** — `opfs::memory::DirectoryHandle::default()` to avoid touching the real filesystem and to keep tests hermetic.
- [ ] **No backend-specific types leak into portable signatures** — function signatures in library code should use `opfs::persistent::{DirectoryHandle, FileHandle, WritableFileStream, Result}` and `opfs::persistent::Error`, not `JsValue` / `std::io::Error` / `String`.
- [ ] **No `write_with_u8_array` or other Safari-incompatible patterns** are reintroduced in the `web` module.
- [ ] **Wasm32 builds do not pull `tokio`** through `opfs` (it is `cfg`-gated, but verify with `cargo tree --target wasm32-unknown-unknown` if size matters).
- [ ] **`keep_existing_data: true` paired with `seek` when appending** — never assume the cursor is at the end of the file.
- [ ] **`remove_entry_with_options(_, &FileSystemRemoveOptions { recursive: true })`** is used whenever the target may be a non-empty directory.
- [ ] **`entries()` consumers use `futures::StreamExt`** — `.next().await` / `.collect().await` — not `for` loops, which would require a `TryStream` import.
- [ ] **`examples/basic_usage.rs` and `examples/advanced_file_ops.rs` still run** on `cargo run --example basic_usage` (native) and `cargo run --target wasm32-unknown-unknown --example basic_usage` (web) after your change.

---

## Quick reference: portable skeleton

```rust
use opfs::persistent::{app_specific_dir, DirectoryHandle, FileHandle, WritableFileStream};
use opfs::persistent;
use opfs::{CreateWritableOptions, GetDirectoryHandleOptions, GetFileHandleOptions, FileSystemRemoveOptions, WriteCommandType, WriteParams};
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};
use futures::StreamExt;

async fn example() -> persistent::Result<()> {
    let root = app_specific_dir().await?;

    // Nested directory
    let sub = root
        .get_directory_handle_with_options("nested", &GetDirectoryHandleOptions { create: true })
        .await?;

    // File
    let mut file = sub
        .get_file_handle_with_options("data.bin", &GetFileHandleOptions { create: true })
        .await?;

    // Write
    let mut w = file
        .create_writable_with_options(&CreateWritableOptions { keep_existing_data: false })
        .await?;
    w.write_at_cursor_pos(b"hello").await?;
    w.write_with_params(&WriteParams {
        command_type: WriteCommandType::Write,
        data: Some(b" world".to_vec()),
        position: Some(5),
        size: None,
    }).await?;
    w.close().await?;

    // Read
    let all = file.read().await?;
    let head = file.read_range(0..5).await?;
    let _ = file.size().await?;

    // List
    let mut s = sub.entries().await?;
    while let Some(item) = s.next().await {
        let (name, _entry) = item?;
        let _ = name;
    }

    // Recursive remove
    sub.remove_entry_with_options("data.bin", &FileSystemRemoveOptions { recursive: true }).await?;
    root.remove_entry_with_options("nested", &FileSystemRemoveOptions { recursive: true }).await?;

    Ok(())
}
```
