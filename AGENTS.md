# Agent Instructions

## 1. Project Overview

**egakareta** is a high-performance 3D rhythm game engine written in Rust. It utilizes:

- **Rust (2021 Edition)** as the core programming language.
- **wgpu** for cross-platform, hardware-accelerated 3D rendering.
- **egui** for the user interface and level editor toolset.
- **wasm-pack** to target WebAssembly (`wasm32`) for the web version.
- **Bun** as the package manager and script runner for frontend/tooling.
- **Supabase** for backend services.

The application compiles to both Native (desktop) and WebAssembly (browser). You **must** ensure changes preserve this dual compatibility. This is the single most important architectural constraint in the repository.

## 2. Tooling and Commands

All primary scripts are managed via `bun` in the `package.json` file. Please use these standardized commands rather than guessing or running underlying binaries directly.

### Build & Run

- **Development Server:** `bun run dev` (Runs a watch process and an HTTP server via `nodemon` and `concurrently`)
- **Web Build:** `bun run build` (Compiles Rust to WASM using `wasm-pack build --target web`)
- **Production Build:** `bun run prod` (Builds and runs the bundled JS output)
- **Watch Mode (Internal):** `bun run watch` (Rebuilds Rust/WGSL on change)

### Linting & Formatting

- **Lint (Clippy):** `bun run lint` (Executes `cargo clippy -- -D warnings` - do not bypass these warnings!)
- **Format Code (Rust + JS/JSON/YAML):** `bun run format` (Runs both `cargo fmt` and `prettier --write .`)
- **Prettier Config:** The `.prettierrc` specifies 4 spaces for most web/tooling files, but 2 spaces for `*.yml`/`*.yaml`. `singleQuote: false` and `semi: true`.

### Testing

- **Run All Tests:** `bun run test` (Uses `cargo nextest run` for significantly faster test execution)
- **Run Single Test:** `cargo nextest run <test_name>` or `cargo test <test_name>`
- **Run Ignored/Benchmark Tests:** `bun run benchmark` (`cargo test benchmark_ -- --ignored`)
- **Test Coverage:** `bun run test:coverage` (Generates `lcov.info` using `cargo llvm-cov nextest`)

## 3. Code Architecture & Layout

Understanding the project structure is crucial for idiomatic changes:

- `src/lib.rs` - The primary library entry point exposing core modules.
- `src/main.rs` - The application binary entry point. Extremely thin wrapper around `egakareta_lib::run_native_app()` (or a no-op on `wasm32`).
- `src/game/` - Core engine logic, simulation, and physics. This must be completely decoupled from rendering and I/O to allow headless simulation.
- `src/platform/` - Platform-specific implementations (e.g., `web_runtime.rs` vs `native_runtime.rs`). Contains the abstraction layer over Winit, file storage (`rexie` for indexedDB on Web, `directories` on Native), and audio (`rodio`/`symphonia` vs Web Audio API).
- `src/state/` - Manages the high-level game and editor state.
- `src/editor_ui/` & `src/editor_domain/` - Egui-based UI components and editor state management.
- `src/types.rs` - Core data types (like `LevelObject`), serialization details, and default value generators for `serde`.
- `assets/` - Static assets, textures, and default data to be embedded or loaded.
- `shader.wgsl` - The core WebGPU shader for rendering the game world.

## 4. Code Style & Conventions

### Platform Duality (WASM vs Native)

- Use conditional compilation `#[cfg(target_arch = "wasm32")]` and `#[cfg(not(target_arch = "wasm32"))]` as needed to separate platform-specific logic. However, strive to keep the logic as unified as possible and prefer libraries that support both targets seamlessly.
- Instead of standard library `std::time`, use `web_time` to guarantee compatibility across platforms.

### Error Handling

- The codebase relies heavily on the `Result<T, String>` pattern for broad error handling across boundaries (e.g., `src/platform/io.rs`).
- Avoid creating deeply nested custom `enum` error types unless a subsystem specifically requires matching against distinct error variants. Use `.map_err(|e| e.to_string())` to propagate third-party errors outwards seamlessly.
- Do not use `unwrap()` or `expect()` outside of test modules unless a failure is absolutely unrecoverable and indicates a fatal programmer logic error. Prefer bubbling up errors.

### Type Definitions & Math

- Use `glam` for all linear algebra and math structures (e.g., `Vec3`, `Mat4`). Ensure types derive `bytemuck::Pod, bytemuck::Zeroable` when pushed to WebGPU buffers.
- For serialization, heavily utilize `serde`. Define defaults and skipping logic cleanly as standalone functions (refer to `src/types.rs` for examples like `fn default_spawn_position() -> [f32; 3]`).

### Naming Conventions

- Rust standard naming: `snake_case` for variables, functions, and modules; `PascalCase` for structs, enums, and traits.
- Constant values should be `UPPER_SNAKE_CASE` (e.g., `CURRENT_LEVEL_FORMAT_VERSION`).
- Boolean states or predicate functions should typically start with `is_` or `has_`.

### Testing Style

- Put unit tests at the bottom of the module in `#[cfg(test)] mod tests { ... }` or in an adjacent `tests.rs` file (e.g., `src/game/tests.rs`).
- Do not use strict equality for floating point numbers (`f32`). Write or use custom test helpers like `fn approx_eq(a: f32, b: f32, eps: f32)` as seen in `src/game/tests.rs`.
- Ensure tests run correctly when compiled for the web (if possible) or strictly target native environments using `#[cfg(not(target_arch = "wasm32"))]` if testing system IO or native-only dependencies.
- Make tests hermetic. Do not rely on shared global state.

## 5. Development Workflow Guidelines

1. **Understand First:** Before modifying logic, always read `Cargo.toml` and `package.json` to understand the build chain. Check `src/types.rs` or `src/game/mod.rs` to see how the subsystem fits together.
2. **Self-Verification:** Before finalizing a task, ensure the code builds (`bun run build`) and passes all strict lints (`bun run lint`). Unverified code will fail the CI.
3. **Reference Existing Code:** If implementing UI, review existing `egui` implementations in `src/editor_ui/`. For core mechanics, review `src/game/physics.rs`.
4. **No Unrelated Formatting Changes:** Never format or refactor code outside the immediate scope of the feature or bug fix. Let `bun run format` handle consistency.
5. **Security & Integrity:** Never commit secrets, local debug paths, or bypassing comments like `#[allow(dead_code)]` unless absolutely unavoidable and well-justified.
6. **Overwrite Database:** This project has not been released. If you need to make breaking changes to the database schema, do not worry about migration scripts. You can simply update the schema and reset the database as needed during development.

---

By meticulously following these guidelines, you ensure `egakareta` remains robust, maintainable, and highly performant across all supported platforms.
