# Copilot Instructions

## Project Overview

`spotify-player` is a terminal Spotify client (requires Spotify Premium) written in Rust. It supports playback control, Spotify Connect, direct streaming via librespot, synced lyrics, desktop notifications, OS media controls, album art rendering, a daemon mode, and a full CLI interface.


### Key modules in `spotify_player/src/`

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point; wires threads/tasks; CLI arg parsing; logging init |
| `state/` | Shared app state (`Arc<State>`): UI, player data, library caches |
| `state/model.rs` | Core domain types: `Track`, `Album`, `Artist`, `Playlist`, `Playback`, etc. |
| `state/player.rs` | `PlayerState`: current playback, devices, queue, progress estimation |
| `state/data.rs` | `AppData`: user library, TTL memory caches, file-cache persistence |
| `state/ui/` | `UIState`: page history stack, popup state, key buffer, count prefix |
| `client/mod.rs` | `AppClient`: Spotify API calls, session management |
| `client/request.rs` | `ClientRequest` and `PlayerRequest` enums (async message types) |
| `client/handlers.rs` | Tokio task: receives `ClientRequest` from channel, dispatches API calls |
| `config/mod.rs` | `Configs` loaded once into a `OnceLock`; accessed via `config::get_config()` |
| `config/keymap.rs` | Default keybindings and key sequence lookup |
| `config/theme.rs` | Theme definitions and style resolution |
| `command.rs` | `Command` enum (all TUI commands) and `Action` / `CommandOrAction` |
| `key.rs` | `Key` and `KeySequence` types; vim-style key parsing (`C-x`, `M-x`) |
| `event/mod.rs` | crossterm input loop; routes key/mouse events to page or popup handler |
| `event/page.rs` | Key event dispatch per page type |
| `event/popup.rs` | Key event dispatch for popup overlays |
| `ui/mod.rs` | ratatui render loop; main layout dispatch |
| `ui/page.rs` | Render functions for each page type |
| `ui/playback.rs` | Render playback bar (progress, metadata, controls) |
| `ui/popup.rs` | Render popup overlays |
| `ui/streaming.rs` | FFT audio visualizer: `VisualizationSink`, `VisBands`, bar chart render |
| `streaming.rs` | librespot connection setup; audio backend configuration (feature-gated) |
| `cli/` | Unix socket server and client for inter-process CLI commands |
| `auth.rs` | OAuth scopes and librespot credential/session building |
| `media_control.rs` | OS media key integration via `souvlaki` (feature-gated) |

### Concurrency model

The app runs multiple OS threads that communicate via `flume` channels and `Arc<State>`:

- **Event thread** — blocking `crossterm::event::read()` loop
- **UI thread** — poll-based ratatui render loop
- **Tokio runtime** — async tasks: client handler, socket listener, streaming
- **Player-event watcher** — polls librespot playback state, sends `ClientRequest`s
- **Media-control thread** — OS media key events (feature-gated)

Event/UI threads never call async functions directly. They send a `ClientRequest` over a `flume` channel and the async handler updates `Arc<State>`.

### Feature flags

| Feature | Effect |
|---|---|
| `streaming` | Enables librespot playback, Spotify Connect, and audio visualization |
| `rodio-backend` | Default audio sink via rodio |
| `alsa-backend`, `pulseaudio-backend`, `portaudio-backend`, `jackaudio-backend`, `rodiojack-backend`, `sdl-backend`, `gstreamer-backend` | Alternative audio sinks |
| `media-control` | OS media key integration |
| `image` | Album art rendering |
| `sixel` | Sixel terminal image protocol (extends `image`) |
| `notify` | Desktop notifications |
| `daemon` | Daemonize mode |
| `fzf` | Fuzzy search |

Default features: `rodio-backend` + `media-control`. Gate code with `#[cfg(feature = "...")]`.

---

## Rust practices after making changes

### 1. Run Clippy

- check clippy with all features enabled to catch any issues in streaming, media control, or image code paths:

```sh
cargo clippy --features streaming,rodio-backend,media-control,image,notify,daemon,fzf -- -D warnings
```

- also check without features to catch any issues in the core code paths:

```sh
cargo clippy --no-default-features
```

To fix warnings with no features, you may need to add `#[allow(dead_code)]` or `#[allow(unused_variables)]` to some functions or variables that are only used in feature-gated code paths.

All warnings are errors in CI. Fix every new lint before committing.

### 2. Run the formatter

```sh
cargo fmt --all
```

CI checks formatting with `cargo fmt --all -- --check`. Always format before pushing.

### 3. Error handling conventions

- Return `anyhow::Result<T>` from all fallible functions.
- Attach context with `.context("...")` or `.with_context(|| ...)`.
- Use `anyhow::bail!("...")` for early exits.
- Format error chains with `{err:#}` (alternate Display), never `{err}`.
- At async task boundaries, log errors and continue — never let a single failure crash a long-running task:
  ```rust
  if let Err(err) = do_something().await {
      tracing::error!("Failed to ...: {err:#}");
  }
  ```

### 4. Logging conventions

Use the `tracing` crate exclusively. Never use `println!`, `eprintln!`, or the `log` crate directly.

```rust
tracing::info!("...");
tracing::warn!("...: {err:#}");
tracing::error!("...: {err:#}");
tracing::debug!("{:?}", value);
```

### 5. No unsafe code

`unsafe_code = "deny"` is set at the workspace level. Do not add `unsafe` blocks.

---

## Keeping documentation up to date

`README.md` and `docs/config.md` are the primary user-facing references. Update them whenever a change affects user-visible behaviour:

- **New feature or config option** — add a description in the relevant section of `README.md` (`Features`, `Configuration`, etc.); document the config field in `docs/config.md`; and update the example config in `examples/app.toml` if applicable.
- **Changed or removed behaviour** — update any affected tables, command descriptions, or usage examples in both `README.md` and `docs/config.md`.
- **New feature flag** — add it to the feature-flags table in `README.md` with a short description.
- **New CLI subcommand** — document it under the CLI section of `README.md`.

Always keep `.github/copilot-instructions.md` up to date if there are significant changes to the project structure, architecture, or conventions.

### Adding a new `Command`

1. Add the variant to `Command` in `command.rs` and update `Command::desc()`.
2. Add a default keybinding in `config/keymap.rs`.
3. Update the command table in `README.md`.
