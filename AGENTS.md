# Agent Instructions

## 1. Project Overview

**egakareta** is a 3D rhythm game written in Rust. It utilizes:

- **Rust (2024 Edition)** as the core programming language.
- **wgpu** for cross-platform, hardware-accelerated 3D rendering.
- **egui** (with `egui-phosphor` icons) for the user interface.
- **wasm-pack** to target `wasm32-unknown-unknown` for the web version.
- **Bun** as the package manager for frontend.
- **mise** for tooling and task orchestration.
- **Cloudflare Pages (wrangler)** for the web deployment target and dev server.
- **Supabase** for backend services (auth, database, storage).

The application compiles to both Native (desktop) and WebAssembly (browser). You **must** ensure changes preserve this dual compatibility. This is the single most important architectural constraint in the repository. Use `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` for platform-specific branches and prefer libraries that support both targets.

## 2. Tooling and Commands

All primary scripts are managed via `mise` in the `mise.toml` file. Use these standardized commands rather than guessing or running underlying binaries directly.

### Build & Run

- **Development Server:** `mise dev` (`bun scripts/dev.ts`)
    - `wrangler pages dev` serving from `./dist` on `http://127.0.0.1:8788`. If this is running, you have access to local Cloudflare services (KV, R2, D1, Durable Objects, and Workflows) for this app via the Explorer API.
    - Fetch the OpenAPI schema from http://127.0.0.1:8788/cdn-cgi/explorer/api to discover available operations. Use these endpoints to list, query, and manage local resources during development.
- **Web Build:** `mise build --dev` (`bun scripts/build.ts --dev`)
- **CLI Tool:** The `egb` binary (`crates/egb/src/main.rs`) is a standalone level metadata conversion helper (JSON ↔ binary `.egb`). Build it with `cargo build -p egb`.

### Linting & Formatting

- **Lint:** `mise lint` — executes Clippy. **Never bypass these warnings!** Clippy denials are treated as hard errors in CI.
- **Format Code:** `mise format` — runs `cargo fmt` then oxfmt.
- **License Headers:** `mise license:add` / `mise license:check` — ensures every source file has the AGPLv3/commercial copyright header. The lefthook pre-commit hook runs `license:check` and `format` in parallel.

### Testing

- **Run All Tests:** `mise test`
- **Run Single Test:** `cargo test <test_name>`
- **Run Coverage:** `mise test:coverage`
- **Database Tests:** `mise test:db` → `supabase test db`

### Type Generation

- **Generate Types:** `mise typegen` regenerates wrangler and supabase types.

## 3. Code Architecture & Layout

- `crates/eg-app/src/platform/runtime.rs`: `Runtime` is the current frame owner and owns only a `bevy_app::App`. `Runtime::run_app_frame(raw_input)` queues platform input, runs App Update for the frame-local egui view, then lets App PostUpdate drive `StateSchedule` and the post-gameplay `RenderExtractSchedule`; presentation reads App-owned render resources, and no legacy State/update/render callback or persistent UI snapshot remains.
- `crates/eg-state/src/events/`: Bevy `Message` taxonomy for user intents and async results. New Bevy-facing command work should add a semantic `Message` type whose handler mutates App-world resources directly or writes a typed request resource for a scheduled system to consume. Runtime should only handle platform ingress and presentation; typed requests are consumed inside `StateSchedule` unless they require an unavoidable external service boundary.
- `crates/eg-state/src/commands.rs`: `AppCommand` is the legacy UI/input intent enum. It is translated at App-world ingress into typed Bevy messages by `eg-state::events` and `state::input_bridge`; do not add new top-level variants when an editor-specific `EditorCommand` + message bridge is enough.
- `crates/eg-codec/src/lib.rs`: Binary level format (`EGB1` magic, versioned). Handles encoding/decoding `LevelMetadata` ↔ compressed binary (Zstd, CBOR). Supports `ObjectRun` run-length encoding for compact object streams.
- `crates/eg-level/src/import_export.rs`: Public API for level import/export: JSON ↔ binary conversion, `.egz` archive handling, format normalization.
- `crates/eg-game/src/`: Core Engine (headless, no I/O). Must remain completely decoupled from rendering and I/O. It exposes an ECS core (`GamePlugin`, gameplay components/resources/systems); gameplay authority and timeline preview are App/ECS-world driven, with checkpoint values kept as plain data.
- `crates/eg-platform/src/`: Platform services shared by app/state/audio code.
- `crates/eg-state/src/`: Application State & Editor Logic. This crate owns the Bevy schedule/resource boundary (`StateSchedule`, `StatePlugins`, async/result bridges, and render extraction) plus the App-owned editor/session resources.
- `crates/eg-state/src/state/editor_command.rs`: `EditorCommand` sub-enum for all editor-specific commands. New editor features should add variants here, not in `AppCommand`.
- `crates/eg-state/src/state/input_bridge.rs`: Drains queued platform `InputEvent`s into App-world resources and typed Bevy messages before command handlers run.
- `crates/eg-state/src/state/gameplay_world.rs` and `events/misc.rs`: App-world gameplay transition, pause, practice, and Escape systems. Keep compatibility logic behind typed resources/messages rather than adding a State owner.
- `crates/eg-editor-domain/src/`: Editor Domain Logic
- `crates/eg-ui/src/`: Egui UI Components
- `crates/eg-mesh/src/`: Mesh Generation
- `crates/egb/src/main.rs`: A standalone CLI tool for level metadata conversion:
    - `egb decode <input.egb> [output.json]` — binary → JSON
    - `egb encode <input.json> [output.egb]` — JSON → binary
    - `crates/egb/tests/egb_tests.rs`: Integration tests.
- `assets/blocks/`: Block files. Embedded at compile time via `include_dir!`.
- `assets/levels/`: Built-in level directories. Each contains level metadata and audio files.

### Other Files

- `crates/eg-render/src/shader.wgsl` — The core WebGPU shader. Compiled at build time. Defines vertex/fragment shaders for block rendering with texture arrays, color spaces, and line uniforms.
- `crates/eg-app/build.rs` — Build script that bakes `wrangler.jsonc` environment variables into the binary for the configured build environment and generates build-time mesh assets.
- `wrangler.jsonc` — Cloudflare Pages configuration with environment-specific vars.
- `functions/` — Cloudflare Pages Functions (TypeScript serverless backend, Supabase integration).

## 4. Code Style & Conventions

### Platform Duality (WASM vs Native)

- Use conditional compilation `#[cfg(target_arch = "wasm32")]` and `#[cfg(not(target_arch = "wasm32"))]` for platform-specific logic. Keep unified code paths where possible.
- Use `web_time::Instant` instead of `std::time::Instant`.
- The `crate-type` in `Cargo.toml` is `["cdylib", "rlib"]` — `cdylib` for WASM, `rlib` for native testing.
- For any `cfg`-gated struct fields, declare them in **all** branches (use empty tuples or `Option` on the non-applicable target) so the struct layout is consistent.
- Use `opfs` for both WASM and native file I/O.
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
- For serialization, heavily utilize `serde`. Use the `serde_default!` macro family in `src/types.rs` to define defaults (see "Adding a New Serde Field" below).
- Use arrays like `[f32; 3]` for positions/colors when serializing to JSON, rather than glam types directly.

### Refactor Points (full-Bevy migration)

We are migrating from a hand-rolled state machine to a `bevy_ecs` + `bevy_app` core (custom wgpu/egui/rodio/cpal stack kept for now). The current production frame path drives `bevy_app::App::update()` from winit, and App owns the egui presentation resources; the legacy `State` compatibility shell has been deleted; remaining custom winit/wgpu/egui and external-service boundaries are App-owned. A future revision will move to the full Bevy umbrella (`bevy_render`, `bevy_winit`, `bevy_egui`, `bevy_tasks`, `bevy_time` `Time<Fixed>`, etc.). Every site that will eventually be replaced by a full-Bevy feature **MUST** be tagged with a `// REFACTOR POINT(full-bevy): <one-line summary>` doc comment and have a matching entry in [docs/bevy-migration.md](docs/bevy-migration.md). The comment is the canonical pointer; the doc is the registry. When the swap happens, both the comment and the doc entry are removed together.

Please read Bevy skills in `.agents/skills/bevy-*/SKILL.md` for guidance.

### Block Asset Authoring

Built-in block definitions live in `assets/blocks/*.json`. Prefer JSON `geometry.elements` for cuboid or voxel-style shapes. Each cuboid uses 0..16 block-local coordinates:

```json
"geometry": {
	"elements": [{ "from": [4, 0, 4], "to": [12, 16, 12] }],
	"hitboxes": [{ "from": [3, 0, 3], "to": [13, 16, 13] }]
}
```

Blocks can define inline `animation` metadata for reusable idle motion and event effects. Use `idle.spin`, `idle.bob`, and `idle.pulse` for shader-side motion, and `on_consumed` or `on_break` for burst effects:

```json
"animation": {
	"idle": {
		"phase_seed": "position",
		"spin": { "axis": "y", "speed": 1.6 },
		"bob": { "axis": "y", "amplitude": 0.055, "frequency": 2.4 }
	},
	"on_consumed": { "preset": "shatter", "duration_seconds": 0.48 }
}
```

## 5. Testing Style & Conventions

### Test Organization

- Unit tests go in `#[cfg(test)] mod tests { ... }` at the bottom of each source file, or in an adjacent `tests.rs` file (e.g., `crates/eg-game/src/tests.rs`, `crates/eg-mesh/src/tests.rs`).
- Integration tests for the `egb` binary live in `crates/egb/tests/egb_tests.rs`.
- Editor snapshot tests live in `crates/eg-state/src/state/editor_snap_tests.rs`.
- Shader compilation tests live in `crates/eg-render/src/shader_tests.rs`.

### Floating Point Assertions

- Use `approx::assert_abs_diff_eq!(a, b, epsilon = eps)` for assertions and avoid `assert_eq!` for `f32` values unless explicitly exact.
- Default epsilon varies by test domain; physics tests commonly use `1e-6`.

### Async Tests

- Production bootstrap is async (`AppBootstrap::new_native` / `AppBootstrap::new`); headless tests should use `TestApp` and direct App-world resources. Use `#[tokio::test]` only for tests that await actual async services.
- Editor tests use `enter_editor_phase("name")` with minimal test data when built-in level metadata is not needed. Avoid `start_editor(0)` (which loads full built-in data) unless specifically testing built-in levels — it makes tests multi-second.

### Adding a New Serde Field

- Use the `serde_default!` family of macros in `src/types.rs` instead of writing separate `default_*` and `is_default_*` functions:
    - `serde_default!(default_foo: u32 = 42)` — for integers, booleans, enums, strings
    - `serde_default_float!(default_bar: f32 = 1.0)` — for float epsilon comparison
    - `serde_default_array!(default_pos: [f32; 3] = [0.0, 0.0, 0.0])` — for float arrays

### Test Data

- For level export/import tests, set a tiny test music source and populate `local_audio_cache` to avoid expensive built-in audio read/compression.

### Platform-Specific Tests

- Use `#[cfg(not(target_arch = "wasm32"))]` on tests that need filesystem access, native audio, or other platform-specific features.
- WASM smoke tests may run without a compatible GPU adapter — treat graceful startup failure (no panic/crash) as a valid outcome alongside successful first-frame readiness.

## 6. Development Workflow Guidelines

1. **Understand First:** Before modifying logic, always read `Cargo.toml`, `mise.toml` and `package.json` to understand the build chain. Check the relevant crate under `crates/` to see how the subsystem fits together.
2. **Self-Verification:** Before finalizing a task, ensure the code builds (`mise build --dev`) and passes all strict lints (`mise lint`). Unverified code will fail the CI.
3. **Reference Existing Code:** If implementing UI, review existing `egui` implementations in `crates/eg-ui/src/`. For core mechanics, review `crates/eg-game/src/physics.rs`.
4. **No Unrelated Formatting Changes:** Never format or refactor code outside the immediate scope of the feature or bug fix. Let `mise format` handle consistency.
5. **Security & Integrity:** Never commit secrets, local debug paths, or bypassing comments like `#[allow(dead_code)]` unless absolutely unavoidable and well-justified. Use `TODO:` comments to mark temporary workarounds and non-future-proof code.
6. **Overwrite Database:** This project has not been released. If you need to make breaking changes to the database schema, do not worry about migration scripts. You can simply update the schema and reset the database as needed during development. Same goes for `CURRENT_LEVEL_FORMAT_VERSION`, do not worry about backward compatibility for now.

---

By meticulously following these guidelines, you ensure `egakareta` remains robust, maintainable, and highly performant across all supported platforms.
