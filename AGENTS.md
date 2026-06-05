# Agent Instructions

## 1. Project Overview

**egakareta** is a high-performance 3D rhythm game written in Rust. It utilizes:

- **Rust (2021 Edition)** as the core programming language.
- **wgpu** for cross-platform, hardware-accelerated 3D rendering.
- **egui** (with `egui-phosphor` icons) for the user interface.
- **wasm-pack** to target `wasm32-unknown-unknown` for the web version.
- **Bun** as the package manager and script runner for frontend/tooling.
- **Cloudflare Pages (wrangler)** for the web deployment target and dev server.
- **Supabase** for backend services (auth, database, storage).

The application compiles to both Native (desktop) and WebAssembly (browser). You **must** ensure changes preserve this dual compatibility. This is the single most important architectural constraint in the repository. Use `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` for platform-specific branches and prefer libraries that support both targets.

## 2. Tooling and Commands

All primary scripts are managed via `bun` in the `package.json` file. Use these standardized commands rather than guessing or running underlying binaries directly.

### Build & Run

- **Development Server:** `bun scripts/dev.ts` runs wasm-pack and wrangler.
    - `wrangler pages dev` serving from `./dist` on `http://127.0.0.1:8788`. If this is running, you have access to local Cloudflare services (KV, R2, D1, Durable Objects, and Workflows) for this app via the Explorer API.
    - Fetch the OpenAPI schema from http://127.0.0.1:8788/cdn-cgi/explorer/api to discover available operations. Use these endpoints to list, query, and manage local resources during development.
- **Web Build:** `bun run build` — prepares `./dist` with wasm-pack and `scripts/dist.ts`.
- **CLI Tool:** The `egb` binary (`src/bin/egb.rs`) is a standalone level metadata conversion helper (JSON ↔ binary `.egb`). Build it with `cargo build --bin egb`.

### Linting & Formatting

- **Lint (Clippy):** `bun run lint` — executes `cargo clippy -- -D warnings`. **Never bypass these warnings!** Clippy denials are treated as hard errors in CI.
- **Format Code:** `bun run format` — runs `cargo fmt` then `prettier --write .`.
- **License Headers:** `bun run license:add` / `bun run license:check` — ensures every source file has the AGPLv3/commercial copyright header. The lefthook pre-commit hook runs `license:check` and `format` in parallel.

### Testing

- **Run All Tests:** `bun run test`
- **Run Single Test:** `cargo test <test_name>`
- **Run Coverage:** `bun run test:coverage`
- **Database Tests:** `bun run test:db` → `supabase test db`

### Type Generation

- **Generate Types:** `bun run typegen` regenerates wrangler and supabase types.

## 3. Code Architecture & Layout

- `src/commands.rs`: `AppCommand` is the central action dispatch pattern. Every user intent (keyboard shortcut, UI button, mouse click) becomes an `AppCommand` variant, routed through `State::dispatch()`. This decouples input from execution and enables replay/macro/test harness support.
- `src/level_codec.rs`: Binary level format (`EGB1` magic, versioned). Handles encoding/decoding `LevelMetadata` ↔ compressed binary (Zstd, CBOR). Supports `ObjectRun` run-length encoding for compact object streams.
- `src/import_export_service.rs`: Public API for level import/export: JSON ↔ binary conversion, `.egz` archive handling, format normalization.
- `src/game/`: Core Engine (headless, no I/O). Must remain completely decoupled from rendering and I/O. It only operates on `GameState` and `LevelObject` data. This allows headless simulation in tests and the editor preview.
- `src/platform/`: Platform Abstraction Layer
- `src/state/`: Application State & Editor Logic
- `src/editor_domain/`: Editor Domain Logic
- `src/editor_ui/`: Egui UI Components
- `src/mesh/`: Mesh Generation
- `src/bin/egb.rs`: A standalone CLI tool for level metadata conversion:
    - `egb decode <input.egb> [output.json]` — binary → JSON
    - `egb encode <input.json> [output.egb]` — JSON → binary
    - `tests/egb_tests.rs`: Integration tests.
- `assets/blocks/`: Block files. Embedded at compile time via `include_dir!`.
- `assets/levels/`: Built-in level directories. Each contains level metadata and audio files.

### Other Files

- `src/shader.wgsl` — The core WebGPU shader. Compiled at build time. Defines vertex/fragment shaders for block rendering with texture arrays, color spaces, and line uniforms.
- `build.rs` — Build script that bakes `wrangler.jsonc` environment variables into the binary for the configured build environment.
- `wrangler.jsonc` — Cloudflare Pages configuration with environment-specific vars.
- `functions/` — Cloudflare Pages Functions (TypeScript serverless backend, Supabase integration).

## 4. Code Style & Conventions

### Platform Duality (WASM vs Native)

- Use conditional compilation `#[cfg(target_arch = "wasm32")]` and `#[cfg(not(target_arch = "wasm32"))]` for platform-specific logic. Keep unified code paths where possible.
- Use `web_time::Instant` (aliased as `PlatformInstant`) instead of `std::time::Instant` everywhere.
- The `crate-type` in `Cargo.toml` is `["cdylib", "rlib"]` — `cdylib` for WASM, `rlib` for native testing.
- For any `cfg`-gated struct fields, declare them in **all** branches (use empty tuples or `Option` on the non-applicable target) so the struct layout is consistent.
- WASM storage uses IndexedDB via `rexie`. Native storage uses `directories` for app data paths.
- WASM file export triggers a browser download via an anchor element click. Native writes to disk with `std::fs`.
- WASM logging uses `gloo_console`. Native uses the `log` crate + `env_logger`.
- WASM panics are routed through `console_error_panic_hook`.

### Imports & Module Organization

- Group imports in this order: (1) standard library, (2) external crates, (3) internal `crate::` modules.
- Use `use crate::` for internal imports, never absolute paths from the crate root.
- Re-export public items from `lib.rs` using `pub use` to define the crate's public API.
- Internal modules are declared as `mod` (private) in `lib.rs`; only expose what is needed externally.
- Use `pub(crate)` for items that should be visible across modules within the crate but not externally.

### Profiling

- The project uses `puffin` for frame-level profiling with `puffin_egui` for the in-app profiler UI.
- Wrap performance-sensitive scopes with `puffin::profile_scope!("name")`.

### Type Definitions & Math

- Use `glam` for all linear algebra and math structures (e.g., `Vec3`, `Mat4`). Ensure types derive `bytemuck::Pod, bytemuck::Zeroable` when pushed to WebGPU buffers.
- For serialization, heavily utilize `serde`. Define defaults and skipping logic cleanly as standalone functions (refer to `src/types.rs` for examples like `fn default_spawn_position() -> [f32; 3]`).
- Use arrays like `[f32; 3]` for positions/colors when serializing to JSON, rather than glam types directly.

## 5. Testing Style & Conventions

### Test Organization

- Unit tests go in `#[cfg(test)] mod tests { ... }` at the bottom of each source file, or in an adjacent `tests.rs` file (e.g., `src/game/tests.rs`, `src/mesh/tests.rs`).
- Integration tests for the `egb` binary live in `tests/egb_tests.rs`.
- Editor snapshot tests live in `src/state/editor_snap_tests.rs`.
- Shader compilation tests live in `src/state/shader_tests.rs`.

### Floating Point Assertions

- Never use `assert_eq!` for `f32` values. Use `crate::test_utils::approx_eq(a, b, eps)` and `crate::test_utils::assert_approx_eq(a, b, eps)`.
- Default epsilon varies by test domain; physics tests commonly use `1e-6`.

### Async Tests

- State construction is `async` (e.g., `State::new_test().await`). Use `pollster::block_on(...)` to run async tests synchronously.
- Editor tests use `enter_editor_phase("name")` with minimal test data when built-in level metadata is not needed. Avoid `start_editor(0)` (which loads full built-in data) unless specifically testing built-in levels — it makes tests multi-second.

### Test Data

- For level export/import tests, set a tiny test music source and populate `local_audio_cache` to avoid expensive built-in audio read/compression.

### Platform-Specific Tests

- Use `#[cfg(not(target_arch = "wasm32"))]` on tests that need filesystem access, native audio, or other platform-specific features.
- WASM smoke tests may run without a compatible GPU adapter — treat graceful startup failure (no panic/crash) as a valid outcome alongside successful first-frame readiness.

## 6. Development Workflow Guidelines

1. **Understand First:** Before modifying logic, always read `Cargo.toml` and `package.json` to understand the build chain. Check `src/types.rs` or `src/game/mod.rs` to see how the subsystem fits together.
2. **Self-Verification:** Before finalizing a task, ensure the code builds (`bun run build`) and passes all strict lints (`bun run lint`). Unverified code will fail the CI.
3. **Reference Existing Code:** If implementing UI, review existing `egui` implementations in `src/editor_ui/`. For core mechanics, review `src/game/physics.rs`.
4. **No Unrelated Formatting Changes:** Never format or refactor code outside the immediate scope of the feature or bug fix. Let `bun run format` handle consistency.
5. **Security & Integrity:** Never commit secrets, local debug paths, or bypassing comments like `#[allow(dead_code)]` unless absolutely unavoidable and well-justified.
6. **Overwrite Database:** This project has not been released. If you need to make breaking changes to the database schema, do not worry about migration scripts. You can simply update the schema and reset the database as needed during development. Same goes for `CURRENT_LEVEL_FORMAT_VERSION`, do not worry about backward compatibility for now.

---

By meticulously following these guidelines, you ensure `egakareta` remains robust, maintainable, and highly performant across all supported platforms.
