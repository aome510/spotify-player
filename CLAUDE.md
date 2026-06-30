# CLAUDE.md

This file guides Claude Code when working in this repository.

## Project Overview

`spotify-player` is a terminal Spotify client (requires Spotify Premium) written in Rust. It supports playback control, Spotify Connect, direct streaming via librespot, synced lyrics, desktop notifications, OS media controls, album art rendering, a daemon mode, and a full CLI interface.

## Architecture

### Key modules in `spotify_player/src/`

| Module                        | Responsibility                                                              |
| ----------------------------- | --------------------------------------------------------------------------- |
| `main.rs`                     | Entry point; wires threads/tasks; CLI arg parsing; logging init             |
| `state/`                      | Shared app state (`Arc<State>`): UI, player data, library caches, queue     |
| `state/model.rs`              | Core domain types: `Track`, `Album`, `Artist`, `Playlist`, `Playback`, etc. |
| `state/player.rs`             | `PlayerState`: current playback, devices, queue, progress estimation        |
| `state/data.rs`               | `AppData`: user library, TTL memory caches, file-cache persistence          |
| `state/ui/`                   | `UIState`: page history stack, popup state, key buffer, count prefix        |
| `client/mod.rs`               | `AppClient`: Spotify API calls, session management                          |
| `client/request.rs`           | `ClientRequest` / `PlayerRequest` enums (async message types)               |
| `client/handlers.rs`          | Tokio task: receives `ClientRequest`, dispatches API calls                  |
| `config/mod.rs`               | `Configs` loaded once into a `OnceLock`; read via `config::get_config()`    |
| `config/keymap.rs`            | Default keybindings and key sequence lookup                                 |
| `config/theme.rs`             | Theme definitions and style resolution                                      |
| `command.rs`                  | `Command` enum (all TUI commands), `Action` / `CommandOrAction`             |
| `key.rs`                      | `Key` / `KeySequence` types; vim-style key parsing (`C-x`, `M-x`)           |
| `event/mod.rs`                | crossterm input loop; routes events to page or popup handler                |
| `event/{page,popup}.rs`       | Key event dispatch per page / popup overlay                                 |
| `ui/mod.rs`                   | ratatui render loop; main layout dispatch                                   |
| `ui/{page,playback,popup}.rs` | Render functions for pages, playback bar, popups                            |
| `ui/streaming.rs`             | FFT audio visualizer: `VisualizationSink`, `VisBands`, bar chart            |
| `streaming.rs`                | librespot connection + audio backend setup (feature-gated)                  |
| `cli/`                        | Unix socket server and client for inter-process CLI commands                |
| `auth.rs`                     | OAuth scopes and librespot credential/session building                      |
| `media_control.rs`            | OS media key integration via `souvlaki` (feature-gated)                     |

### Concurrency model

Multiple OS threads communicate via `flume` channels and `Arc<State>`:

- **Event thread** — blocking `crossterm::event::read()` loop
- **UI thread** — poll-based ratatui render loop
- **Tokio runtime** — async tasks: client handler, socket listener, streaming
- **Player-event watcher** — polls librespot playback state, sends `ClientRequest`s
- **Media-control thread** — OS media key events (feature-gated)

Event/UI threads never call async functions directly. They send a `ClientRequest` over a `flume` channel; the async handler updates `Arc<State>`.

### Feature flags

| Feature                                                                                                                                 | Effect                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `streaming`                                                                                                                             | librespot playback, Spotify Connect, audio visualization |
| `rodio-backend`                                                                                                                         | Default audio sink (rodio)                               |
| `alsa-backend`, `pulseaudio-backend`, `portaudio-backend`, `jackaudio-backend`, `rodiojack-backend`, `sdl-backend`, `gstreamer-backend` | Alternative audio sinks                                  |
| `media-control`                                                                                                                         | OS media key integration                                 |
| `image`                                                                                                                                 | Album art rendering                                      |
| `sixel`                                                                                                                                 | Sixel terminal image protocol (extends `image`)          |
| `pixelate`                                                                                                                              | Pixelated image rendering fallback (extends `image`)     |
| `notify`                                                                                                                                | Desktop notifications                                    |
| `daemon`                                                                                                                                | Daemonize mode (implies `streaming`)                     |
| `fzf`                                                                                                                                   | Fuzzy search                                             |

Default: `rodio-backend` + `media-control`. Gate feature-specific code with `#[cfg(feature = "...")]`.

## Verifying changes

CI treats all warnings as errors. Before committing, run what CI runs:

```sh
cargo fmt --all                      # CI checks: cargo fmt --all -- --check
cargo test --no-default-features --features rodio-backend,media-control,image,notify,fzf
cargo clippy --no-default-features --features rodio-backend,media-control,image,notify,fzf -- -D warnings
cargo clippy --no-default-features -- -D warnings   # core paths, no features
```

When fixing no-feature clippy warnings, you may need `#[allow(dead_code)]` / `#[allow(unused_variables)]` on items only used in feature-gated paths. If you touch `daemon`/`streaming` code, add `daemon` to the feature list above to lint those paths too.

## Conventions

### Error handling

- Return `anyhow::Result<T>` from fallible functions.
- Add context with `.context("...")` / `.with_context(|| ...)`; early-exit with `anyhow::bail!("...")`.
- Format error chains with `{err:#}` (alternate Display), never `{err}`.
- At async task boundaries, log and continue — never let one failure crash a long-running task:
  ```rust
  if let Err(err) = do_something().await {
      tracing::error!("Failed to ...: {err:#}");
  }
  ```

### Logging

Use `tracing` exclusively — never `println!`, `eprintln!`, or the `log` crate.

```rust
tracing::info!("...");
tracing::error!("...: {err:#}");
tracing::debug!("{value:?}");
```

### Comments and doc comments

- Reserve comments for non-obvious intent: invariants, ordering constraints, workarounds, edge cases.
- Do not narrate the implementation. A doc comment states a type/function's purpose and contract (what callers need to know); leave the mechanics — which branch does what, field-by-field behaviour, control flow — to the code itself. Implementation rationale belongs in a focused inline comment at the relevant line, not in the doc comment.
- Keep comments concise and clear, avoid long paragraphs and try to keep comments up to date with code changes.

## Keeping docs up to date

`README.md` and `docs/config.md` are the primary user-facing references. Update them on any user-visible change:

- **New feature / config option** — describe it in `README.md` (`Features`, `Configuration`, …), document the field in `docs/config.md`, and update `examples/app.toml` if applicable.
- **Changed / removed behaviour** — update affected tables, command descriptions, and usage examples in both files.
- **New feature flag** — add it to the feature-flags table in `README.md`.
- **New CLI subcommand** — document it under the CLI section of `README.md`.

Keep `.github/copilot-instructions.md` and this `CLAUDE.md` in sync when project structure, architecture, or conventions change significantly.

### Adding a new `Command`

1. Add the variant to `Command` in `command.rs` and update `Command::desc()`.
2. Add a default keybinding in `config/keymap.rs`.
3. Update the command table in `README.md`.

## Writing PR descriptions

Base the description on the actual branch diff (`git diff master...<branch>`), not assumptions. Keep it clear and concise:

- **Summary** — 2-4 sentences: what problem the change solves and the approach. State the _why_ (the prior behaviour / bug) before the _what_.
- **Changes** — a bullet per logical change, each tagged with the affected module/file. Lead with the user-facing or architectural change, not mechanical edits.
- Prefer plain prose over filler; omit empty sections. Add a short **Notes** section only for non-obvious trade-offs or follow-ups.
