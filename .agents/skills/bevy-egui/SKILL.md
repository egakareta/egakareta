---
name: bevy-egui
description: "Use this skill when embedding an `egui` UI inside a Bevy 0.19 game with the `bevy_egui 0.41` plugin. Covers the multi-pass `EguiPrimaryContextPass` schedule, additional `EguiMultipassSchedule` contexts (multiple windows, split screen, world space), manual begin/end passes, Bevy asset textures via `EguiUserTextures`, custom paint callbacks into the Egui render pass, and run conditions for absorbing input while Egui is focused. Triggers on: egui, bevy_egui, debug UI, editor, inspector, dev panel, imGUI, immediate mode, side panel, paint callback, multi-pass, user textures, secondary window, world-space UI."
argument-hint: "Describe the egui integration (single context, multi-pass, paint callback, textures, input)"
---

# Bevy 0.19 + `bevy_egui 0.41`

## Quick Start

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, spawn_camera)
        .add_systems(EguiPrimaryContextPass, ui_example_system)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn ui_example_system(mut contexts: EguiContexts) -> Result {
    egui::Window::new("Hello").show(contexts.ctx_mut()?, |ui| {
        ui.label("world");
    });
    Ok(())
}
```

`EguiPlugin::default()` is multi-pass; UI systems go into `EguiPrimaryContextPass`, not `Update`. The plugin auto-attaches a `PrimaryEguiContext` to the first camera unless you opt out via `EguiGlobalSettings::auto_create_primary_context = false`.

---

## Core Concepts

### How `bevy_egui` fits into a Bevy 0.19 frame

`EguiPlugin` owns a `bevy_ecs::Message<EguiInputEvent>` and, per context, a `Component` bundle. Every frame:

1. `EguiPreUpdateSet::InitContexts` registers newly created cameras (`WindowToEguiContextMap::on_egui_context_added_system`) and updates `screen_rect` from the camera viewport.
2. `EguiPreUpdateSet::ProcessInput` (chained `EguiInputSet::InitReading → FocusContext → ReadBevyMessages → WriteEguiEvents`) translates Bevy's `MouseButtonInput`, `MouseWheel`, `KeyboardInput`, `CursorMoved`, `TouchInput`, `PinchGesture`, `Ime`, and `FileDragAndDrop` into `egui::Event`s per context.
3. `EguiPreUpdateSet::BeginPass` calls `ctx.begin_pass` (or skips it for `run_manually` contexts).
4. `EguiPostUpdateSet::EndPass` runs the `EguiPrimaryContextPass` schedule (or each context's `EguiMultipassSchedule`) inside `ctx.run_ui` so widgets can call `egui::Context::request_discard` and re-run.
5. `EguiPostUpdateSet::ProcessOutput` calls `process_output_system` which tessellates paint jobs into `EguiRenderOutput` and drives clipboard / cursor / `RequestRedraw`.
6. The Egui render pass runs after `bevy_core_pipeline::Core2dSystems::MainPass` (2D) or after `bevy_core_pipeline::Core3dSystems::PostProcess` (3D) and before upscaling.

### Contexts and the two schedules

There is exactly one schedule for the primary context, and one unique schedule per additional context:

- `EguiPrimaryContextPass` — `ScheduleLabel` that owns the primary context's UI systems. Multi-pass mandatory in 0.41.
- `EguiMultipassSchedule(InternedScheduleLabel)` — required `Component` on every secondary context (extra windows, split screen, render-to-image, world-space). Each instance **must** use a unique schedule or `run_egui_context_pass_loop_system` will panic.
- `EguiContext` is the owning component; `EguiContextSettings` controls per-context behavior; `EguiInput` / `EguiFullOutput` / `EguiRenderOutput` / `EguiOutput` are the per-frame buffers.

### `EguiContexts` system param

`EguiContexts` is the supported way to talk to egui from systems. It exposes:

- `ctx_mut() -> Result<&mut egui::Context, QuerySingleError>` — entity that has `PrimaryEguiContext`.
- `ctx_for_entity_mut(entity)` / `ctx_for_entities_mut([..])` — by camera entity; use these for non-primary contexts.
- `add_image(handle) -> egui::TextureId`, `remove_image(id)`, `image_id(id) -> Option<egui::TextureId>` — proxies to `EguiUserTextures` (render feature only).

A plain `Query<&mut EguiContext, With<PrimaryEguiContext>>` is the alternative when you want single-entity semantics or work in a generic const-generic schedule (see `split_screen.rs`).

### Manual pass mode

Set `EguiContextSettings::run_manually = true` to take ownership of `EguiInput → ctx.run_ui(..., |ctx| { ... }) → EguiFullOutput`. Useful when you need a custom pass loop, control discard, or call `ctx.run`/`begin_pass`/`end_pass` from inside an existing render step. The verified `run_manually.rs` does this:

```rust
// PreStartup: configure_context_system flips run_manually to true.
fn ui_example_system(
    mut contexts: Query<(&mut EguiContext, &mut EguiInput, &mut EguiFullOutput)>,
) -> Result {
    let (mut ctx, mut egui_input, mut egui_full_output) = contexts.single_mut()?;
    let ctx_ref = ctx.get_mut();
    ctx_ref.memory_mut(|m| m.options.max_passes = NonZero::new(5).unwrap());
    **egui_full_output = Some(ctx_ref.run_ui(egui_input.take(), |ui| {
        egui::Window::new("Hello").show(ui, |ui| {
            ui.ctx().request_discard("Trying to reach max limit");
        });
    }));
    Ok(())
}
```

`EguiPlugin` will skip `begin_pass_system` / `end_pass_system` for any context with `run_manually = true`.

### Asset textures, custom paint, and the render pass

- `EguiUserTextures` is the global registry from `Handle<Image>` / `AssetId<Image>` to `egui::TextureId::User`. `EguiTextureHandle::Strong(handle)` keeps the asset alive; `EguiTextureHandle::Weak(asset_id)` lets Bevy own the lifetime. `free_egui_textures_system` (in `Last`) auto-cleans weak handles on `AssetEvent::Removed`.
- Paint callbacks (`EguiBevyPaintCallback`, `EguiBevyPaintCallbackImpl`, `EguiPipelineKey`) let a callback record GPU work that runs inside the Egui render pass. They are the supported way to draw Bevy meshes or wgpu pipelines from inside an egui window. The `paint_callback.rs` example shows the full `RenderApp` + `SpecializedRenderPipeline` wiring.
- `bevy_egui` ships its own internal shader at `render/egui.wgsl`, registered via `load_internal_asset!` keyed by `render::EGUI_SHADER_HANDLE`. The 2D pass runs after `Core2dSystems::MainPass`; the 3D pass runs after `Core3dSystems::PostProcess` (since 0.41.1). Both run before upscaling.

### Input: run conditions vs absorption

Two complementary patterns:

1. **Run conditions** — gate your own systems so they don't fight Egui:

    ```rust
    use bevy_egui::input::{egui_wants_any_keyboard_input, egui_wants_any_pointer_input};

    app.add_systems(Update, my_movement_system.run_if(not(egui_wants_any_keyboard_input)));
    app.add_systems(Update, my_drag_system.run_if(not(egui_wants_any_pointer_input)));
    ```

2. **Absorption** — toggle `EguiGlobalSettings::enable_absorb_bevy_input_system = true` (default is `false`) to let the plugin drain `KeyboardInput`, `MouseButtonInput`, `MouseWheel`, `TouchInput`, `FileDragAndDrop` after Egui consumes them. Use this only when no other plugin needs an uninterrupted stream of those messages.

The plugin always sets `bevy_window::CursorIcon` per Egui cursor request; disable with `EguiGlobalSettings::enable_cursor_icon_updates = false` if you set cursors yourself.

### `bevy_picking` integration (render feature `picking`)

`EguiPlugin` registers three observers and a `PostUpdate` system:

- `handle_over_system` (on `Pointer<Over>`) inserts `HoveredNonWindowEguiContext`.
- `handle_out_system` (on `Pointer<Out>`) removes it.
- `handle_move_system` (on `Pointer<Move>`) ray-casts the picking mesh and writes `EguiContextPointerPosition.position`.
- `capture_pointer_input_system` writes a fake `PointerHits` so picking never passes through an Egui window when `EguiContextSettings::capture_pointer_input = true` (the default).

To make a 3D mesh pickable as an Egui surface: `entity.add_picking_observers_for_context(context_entity)` (or insert `PickableEguiContext(context)` and the three observers yourself). Only `Mesh3d` / `Mesh2d` are supported; the integration needs UV coordinates on the hit.

---

## Common Patterns

### Multiple windows with separate contexts

From `two_windows.rs`. Spawn a primary camera with `PrimaryEguiContext`, a `Window` for the second screen, and a second camera tagged with `EguiMultipassSchedule::new(SecondWindowContextPass)`:

```rust
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SecondWindowContextPass;

fn create_new_window_system(
    mut commands: Commands,
    mut egui_global_settings: ResMut<EguiGlobalSettings>,
) {
    egui_global_settings.auto_create_primary_context = false;
    commands.spawn((Camera3d::default(), PrimaryEguiContext));
    let second_window_id = commands.spawn(Window { title: "Second window".into(), ..default() }).id();
    commands.spawn((
        Camera3d::default(),
        Camera::default(),
        RenderTarget::Window(WindowRef::Entity(second_window_id)),
        EguiMultipassSchedule::new(SecondWindowContextPass),
        Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

app.add_systems(EguiPrimaryContextPass, ui_first_window_system);
app.add_systems(SecondWindowContextPass, ui_second_window_system);
```

Each extra schedule is rendered on its own camera and UI is fully isolated. Each `EguiMultipassSchedule` **must** use a unique `ScheduleLabel` or `run_egui_context_pass_loop_system` panics.

### Split screen with N player cameras

The pattern from `split_screen.rs` is a generic const-N schedule: every player camera gets its own `PlayerCamera<N>` marker, its own `EguiMultipassSchedule(PlayerCamera::<N>.intern())`, and a generic `ui_example_system::<N>` system registered into `PlayerCamera::<N>` (`Component + ScheduleLabel`). `update_camera_viewports_system` resizes each `Camera.viewport` per frame and toggles `Camera::is_active` based on `PlayersCount`. The primary context (camera order 10) overlays the player count UI on top.

### Render-to-image as an egui image widget

From `render_to_image_widget.rs`. Create a Bevy `Image` with `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_DST`, register it in `EguiUserTextures` with a strong handle, render a 3D scene to it via a `Camera` with `RenderTarget::Image`, then look up its id with `contexts.image_id(&handle).unwrap()` and draw it with `egui::widgets::Image::new((id, [300., 300.]))`. Disable the auto primary context first; the image-target camera must be ordered before the primary camera.

### World-space UI on a mesh

From `paint_callback.rs`. Add a second `Camera2d` with `RenderTarget::Image` and `EguiMultipassSchedule::new(WorldspaceContextPass)`. To draw Bevy meshes inside an egui window, register a `SpecializedRenderPipeline<EguiPipelineKey>` in the `RenderApp`, implement `EguiBevyPaintCallbackImpl`, and inside your UI system call `painter.add(EguiBevyPaintCallback::new_paint_callback(rect, CustomPaintCallback))`. The `info: egui::PaintCallbackInfo` gives you the rect; the `render_entity: RenderEntity` lets you pull Bevy components on the render world.

### Paneling a 3D viewport

From `side_panel.rs`. Add `PrimaryEguiContext` to a dedicated overlay `Camera2d` (no meshes rendered, `RenderLayers::none()`, `clear_color: ClearColorConfig::Custom(Color::NONE)`, `order: 1`). Build a full-viewport `egui::Ui` from `ctx.viewport_rect()`, attach `egui::Panel::left/right/top/bottom`, read the resulting rect sizes, scale by `window.scale_factor()`, and write `Camera.viewport` on the world camera so the world view shrinks to the remaining region. The world camera is spawned separately without `PrimaryEguiContext`.

### Disabling auto primary context

Any time you want full control (multi-window, side panels, render-to-image), set the flag in `Startup` before the first camera spawns:

```rust
fn setup(mut settings: ResMut<EguiGlobalSettings>) {
    settings.auto_create_primary_context = false;
}
```

If you do this and forget to spawn a `PrimaryEguiContext`, `EguiContexts::ctx_mut()` will return `QuerySingleError::NoEntities`.

### Reflecting `EguiGlobalSettings`

`EguiPlugin` calls `app.register_type::<EguiGlobalSettings>()` and `EguiContextSettings`, so you can edit them via the Bevy Remote Protocol / inspector without a recompile.

---

## Gotchas

1. **Multi-pass is now the only stable mode.** `EguiPlugin::default()` enables multi-pass; `EguiPlugin { enable_multipass_for_primary_context: false, .. }` is deprecated. Adding UI systems to `Update` is no longer reliable — always add them to `EguiPrimaryContextPass` (or to the per-context multi-pass schedule).
2. **Don't reuse multi-pass schedules.** Two cameras sharing the same `EguiMultipassSchedule` will panic at runtime: `run_egui_context_pass_loop_system` requires unique schedule labels.
3. **Manual `set_pixels_per_point` no longer scales.** As of 0.40.0 the plugin uses `native_pixels_per_point`; control zoom via the `EguiZoomFactor` component on the context (mirrors `egui::Context::zoom_factor`) and let `Ctrl +` / `Ctrl -` work. `set_zoom_factor` (called from `read_egui_zoom_factor_system`) is the supported write path.
4. **Default `pixels_per_point` is 1.0 if `DefaultPlugins` are missing `WindowPlugin`.** `EguiOutput::default()` returns `pixels_per_point: 1.0`; if the camera viewport is 0-sized the system skips updating `screen_rect` and egui will draw nothing.
5. **Strong vs weak user textures.** `EguiTextureHandle::Strong` increments the asset ref count and you must call `remove_image` to drop it. `EguiTextureHandle::Weak` is cleaned up automatically on `AssetEvent::Removed`; use it when Bevy owns the lifetime (most gameplay textures).
6. **`EguiUserTextures` is extracted to the render world.** If you mutate it from `PostUpdate` after `EguiPostUpdateSet::PostProcessOutput`, the change won't show this frame — it is `ExtractResource`d into the render app. Spawn `add_image` in `Startup` or `PreUpdate` for first-frame correctness.
7. **Picking needs UVs.** `handle_move_system` only succeeds when the ray hit exposes `uv: Some(_)`. Surfaces without UVs (`Plane`, `Cuboid`, etc.) silently won't be pickable as Egui; bake UVs in your mesh.
8. **`capture_pointer_input` defaults to `true`.** Egui captures pointers that hover its windows for picking — game picking goes through the `Pointer<Over/Out/Move>` observers instead. If you want world picking to "win" while an Egui window is also under the pointer, set `EguiContextSettings::capture_pointer_input = false` for that context.
9. **The 3D egui pass is ordered after `PostProcess` (since 0.41.1).** If you write your own `Render` systems that run in `RenderSystems::Queue` or later and inspect the framebuffer, they will see _un-tonemapped_ colors. Custom post-FX that need post-tonemap access must run after `bevy_egui`'s pass.
10. **Disabled features matter.** `manage_clipboard` (default on) gates `EguiClipboard`; `open_url` gates `EguiOutputCommand::OpenUrl` and the `webbrowser` dep; `default_fonts` (default on) registers egui's built-in font; `bevy_ui` (default on) integrates with `bevy_ui_render` and `UiRenderOrder`; `picking` (default on) wires the picking observers. Drop them in `Cargo.toml` to slim the dep graph for shipped builds.
11. **`EguiContexts::ctx()` / `ctx_for_entity()` require `immutable_ctx`.** The `get_mut()` API is the supported path; the immutable borrow is feature-gated and can block when the context is mutably held.
12. **`absorb_bevy_input_system` drains Bevy messages.** If you have another plugin that reads `KeyboardInput` / `MouseButtonInput` / `MouseWheel` independently, keep `enable_absorb_bevy_input_system = false` (the default) and use the `egui_wants_any_*` run conditions instead.

---

## Related

- [bevy-app-plugins](../bevy-app-plugins/SKILL.md) — plugin/schedule composition; required to know where `EguiPrimaryContextPass` lives.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) — strong vs weak handles and `Assets<Image>` lifetimes; relevant for `EguiTextureHandle::Strong`.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) — `SystemSet` chaining; the `EguiPreUpdateSet` / `EguiInputSet` / `EguiPostUpdateSet` enums are useful extension points.
- [bevy-input-actions](../bevy-input-actions/SKILL.md) — semantic action mapping; pair with `egui_wants_any_*` run conditions to gate gameplay.
- [bevy-ui-picking](../bevy-ui-picking/SKILL.md) — the in-house Bevy UI alternative; `UiRenderOrder` decides which system draws on top.
- [bevy-render-extensions](../bevy-render-extensions/SKILL.md) — needed for `EguiBevyPaintCallbackImpl` since paint callbacks run inside the `RenderApp`.
- [bevy-states-scoped-entities](../bevy-states-scoped-entities/SKILL.md) — useful for menu/HUD/menu state transitions where egui panels toggle.
- [bevy-scenes-reflection](../bevy-scenes-reflection/SKILL.md) — `EguiGlobalSettings` and `EguiContextSettings` are `Reflect`, so they can be tweaked via the Remote Protocol.
