## Plan: Custom Playback Integration

A unified **custom queue** system that replaces librespot/spirc-managed playback
with app-managed batched URIs playback. This single mechanism resolves six open
issues that all stem from the same architectural limitation: the app delegates
queue management to Spotify/librespot, which only exposes a ~100-track window
and has no support for client-side sorting, smart shuffle, or single-track
autoplay.

### Issues addressed

| Issue                                                        | Title                                        | How the custom queue fixes it                                                                                                                                  |
| ------------------------------------------------------------ | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [#766](https://github.com/aome510/spotify-player/issues/766) | Not all songs queued for large playlists     | Queue holds the **full** track list; sends batches to Spotify and auto-advances when a batch ends.                                                             |
| [#878](https://github.com/aome510/spotify-player/issues/878) | Sorted playlist doesn't play in sorted order | Queue is built from `tracks` **after** client-side sort/reverse, so playback follows the visible order.                                                        |
| [#378](https://github.com/aome510/spotify-player/issues/378) | Automix — true random from playlist          | When shuffle is active, `advance()` picks a random track from the full list (not the ~100-track spirc window). When a batch ends, a new random batch is drawn. |
| [#767](https://github.com/aome510/spotify-player/issues/767) | Autoplay not working for single tracks       | On `EndOfTrack` with no next track in the queue, the app fetches radio tracks via `radio_tracks()` and appends them, continuing playback.                      |
| [#153](https://github.com/aome510/spotify-player/issues/153) | Enhance / Smart Shuffle                      | Smart-shuffle mode interleaves radio recommendation tracks between the playlist's original tracks inside the custom queue.                                     |
| [#739](https://github.com/aome510/spotify-player/issues/739) | Toggle normal / smart shuffle                | Three-state `Shuffle` cycles: Off → Shuffle → Smart Shuffle → Off. Smart Shuffle activates the interleave logic from #153.                                     |

### Root cause

When `ChooseSelected` fires on a playlist track table
([window.rs#L307-L340](spotify_player/src/event/window.rs#L307-L340)), the code
builds a `Playback::Context(ContextId::Playlist(…), offset)` and hands it to
Spotify via `start_context_playback`. librespot's spirc protocol then determines
the queue window — only ~40–100 tracks. Sorting, smart shuffle, and autoplay are
all impossible because the app doesn't control what comes next.

For `Playback::URIs` (liked songs etc.), the `uri_offset` method
([model.rs#L700-L723](spotify_player/src/state/model.rs#L700-L723)) clips the
list to `tracks_playback_limit` (default 50). The same API limit (≈100 URIs)
applies to any single batch.

### Approach

When the `streaming` feature is active (integrated librespot player), replace
context-based playback with **app-managed batched URIs playback**. A
`CustomQueue` struct stores the full ordered track list; the app sends batches
of `tracks_playback_limit` tracks at a time, and auto-advances to the next
batch when the current one ends. For non-streaming mode (external devices), the
existing `Playback::Context` path is unchanged.

---

### Optionality and transition design

The custom queue is **not a global mode toggle**. It is a per-playback state
that is created or cleared automatically based on context. The guiding principle
is: **the custom queue activates transparently when it can help, and gets out of
the way when it can't.**

#### Activation conditions

The custom queue is created when ALL of the following are true:

1. **Streaming is enabled** — `state.is_streaming_enabled()` returns `true`
   (i.e., the integrated librespot player is the active device).
2. **Config allows it** — `config.app_config.custom_queue_enabled` is `true`
   (default). Setting to `false` permanently disables the custom queue; the app
   behaves exactly as it does today.
3. **Playback starts from a track-table context** — The user triggers
   `ChooseSelected` or `PlayRandom` on a playlist, album, artist top-tracks, or
   liked-songs page. This is the moment the queue is built.

Single-track playback from search results (track _list_, not track _table_)
also creates a 1-track custom queue when conditions 1–2 hold, so that autoplay
radio continuation (#767) can work. If the user has `autoplay: false` in their
device config **and** `custom_queue_enabled: false`, this path is skipped
entirely.

#### When the custom queue is NOT created

- **Streaming disabled** — External devices (phone, desktop client, spotifyd)
  manage their own queue. The app sends `Playback::Context` as today.
- **Config opted out** — `custom_queue_enabled: false`.
- **Show/podcast context** — Episodes use `Playback::Context` since shows have
  a natural sequential order managed by Spotify.

#### Deactivation (clearing the custom queue)

The custom queue is set to `None` when any of these occur:

| Trigger                     | Where                                                                                                                                                    | Why                                                                                |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **New playback started**    | `handle_player_request` for `StartPlayback` — if the request does NOT carry a new custom queue, the old one is cleared.                                  | User started playing from a different context or a search result without autoplay. |
| **Device transfer**         | `handle_player_request` for `TransferPlayback` — always clear.                                                                                           | External device takes over queue management.                                       |
| **External context change** | `retrieve_current_playback` — if the Spotify-reported context URI differs from `custom_queue.source_context` and the change wasn't initiated by the app. | Another device or the Spotify app changed what's playing.                          |
| **Playback disappears**     | `retrieve_current_playback` — `playback` becomes `None`.                                                                                                 | Session ended or device disconnected.                                              |

#### Transition scenarios

**1. Playing a playlist → play from a different playlist**

- Step 3 builds a new `CustomQueue` from the new playlist's tracks.
- The `StartPlayback` request carries the new queue. The handler atomically
  replaces the old one. Seamless — no gap, no stale state.

**2. Playing a playlist → play a single track from search**

- `handle_command_for_track_list_window` sends `Playback::URIs([single_track])`.
- The event thread does **not** create a custom queue (search results don't have
  a meaningful context to batch).
- `handle_player_request` for `StartPlayback` clears old `custom_queue`.
- If `autoplay: true`, a 1-track custom queue is created so that on `EndOfTrack`
  the app can fetch radio tracks and continue. Otherwise playback stops after
  the track.

**3. Playing with custom queue → transfer to external device**

- User picks a device from the device popup → `TransferPlayback(device_id)`.
- `handle_player_request` clears `custom_queue`.
- The external device receives the current Spotify playback state and manages
  its own queue from there. Context info is lost (Spotify sees a URIs playback,
  not a playlist context), but this is acceptable — the external device picks up
  wherever the user left off.

**4. External device is playing → transfer back to integrated player**

- `TransferPlayback` to the integrated device. Custom queue is NOT automatically
  recreated — the app picks up whatever Spotify reports as the current playback.
- If the user then starts a new playlist from the TUI, a new custom queue is
  built as usual.

**5. Custom queue active → user manually adds a track to queue**

- `AddPlayableToQueue` sends the track to the Spotify API, which inserts it
  into the native queue (plays after current track).
- The custom queue does **not** track user-queued tracks. The design relies on
  **batch-end detection by track ID**: the `EndOfTrack` handler only triggers a
  batch transition when the ended track's ID matches the last track in the
  current batch. User-queued tracks have different IDs, so they play
  transparently between batch tracks without confusing the batch manager.
- After user-queued tracks finish, the batch resumes normally.

**6. Custom queue active → user presses NextTrack through a user-queued track**

- `NextTrack` in `handle_player_request`: the custom queue calls `advance()`.
  The Spotify API `next_track()` call plays the user-queued track (since it's at
  the front of the native queue). The custom queue advances its `position`.
  When the user-queued track actually plays, the position is now one ahead of
  reality — but this self-corrects on the next `retrieve_current_playback` poll
  or `EndOfTrack` event. **Alternative simpler design**: `NextTrack` always calls
  `self.next_track(device_id)` and does NOT advance the custom queue position.
  Let `EndOfTrack` be the sole position-tracking mechanism. This avoids the
  user-queued track desync issue entirely. **We choose this simpler design.**

**7. App restart while custom queue was active**

- Custom queue state is in-memory only (`PlayerState` is not persisted).
- On restart, `initialize_playback` picks up the current Spotify playback state.
  No custom queue exists. The user starts fresh — this is acceptable for an MVP.

#### The "batch manager" model

The key simplification is: **the custom queue is a batch manager, not a
per-track position tracker.**

- The queue stores the full track list and knows which batch was last sent to
  Spotify.
- It does NOT try to shadow every track transition. librespot handles
  within-batch next/previous natively.
- The queue only intervenes at **batch boundaries**: when the last track of the
  current batch finishes (`EndOfTrack` with matching ID), it sends the next
  batch.
- `NextTrack` / `PreviousTrack` only intervene when the skip would cross a
  batch boundary. Within a batch, the Spotify API `next_track()` /
  `previous_track()` works as-is.
- User-queued tracks (via `AddPlayableToQueue`) play via the native Spotify
  queue and pass through without disrupting the batch manager.

This keeps the custom queue lightweight, avoids race conditions with the Spotify
API's eventual consistency, and means most of the existing codebase is
untouched.

#### State reconciliation

The custom queue and the Spotify API poll (`retrieve_current_playback`) operate
on different planes:

- **Custom queue** is **write-only from its own perspective**: it decides what
  batch to send next and tells Spotify to play it. It never reads Spotify state
  to update its own position — `EndOfTrack` is the sole position tracker.
- **Spotify API poll** is **read-only from the queue's perspective**: it updates
  the UI (progress bar, currently playing metadata, device list) but never
  mutates the custom queue's position or play order.

They interact only at three **junction points** where the custom queue is
**cleared** (not adjusted):

1. **Context URI mismatch**: `retrieve_current_playback` detects that Spotify's
   reported context URI differs from `custom_queue.source_context`. This means
   an external app/device changed the playback context. Clear the queue.
2. **Playback disappears**: `retrieve_current_playback` gets `None`. Session
   ended. Clear the queue.
3. **Device transfer**: `TransferPlayback` handler. Clear the queue.

No other Spotify state (progress, track position, shuffle flag) causes the
custom queue to update. The queue trusts its own bookkeeping.

##### Mismatch scenarios and responses

| Scenario                                                   | Detection                                                                   | Response                                                          |
| ---------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| User changes context on another device                     | Context URI mismatch in `retrieve_current_playback`                         | Clear custom queue                                                |
| User adds track to native queue                            | Different track ID plays — `EndOfTrack` ID doesn't match batch's last track | No action (user-queued track passes through transparently)        |
| Spotify reports different `currently_playing` mid-batch    | Normal — Spotify poll is eventually consistent                              | Ignore; trust `EndOfTrack` for position                           |
| Network blip causes stale playback data                    | Transient — next poll corrects it                                           | No action                                                         |
| User pauses/resumes on external device                     | Spotify poll updates `is_playing`; custom queue unaffected                  | No action                                                         |
| Shuffle flag out of sync (user toggles on external device) | Not detected — custom queue trusts its own shuffle state                    | Acceptable: custom queue's shuffle mode wins for batch generation |

#### Queue consistency checking

As an additional safety net, the app periodically verifies that the custom
queue's expected next track is still present in Spotify's reported queue. This
catches edge cases where the custom queue and Spotify silently diverge.

**Mechanism**: In `retrieve_current_playback` (or the event watcher), when both `custom_queue`
and `player.queue` (from `current_user_queue` API) are available:

1. Compute the expected next track: `play_order[position + 1]` (if it exists and is
   within the current batch).
2. Check if that track ID appears **anywhere** in `queue.queue` (the Spotify-reported
   upcoming track list).
3. If the expected next track is **not found** in the Spotify queue, the custom
   queue is out of sync. Call `truncate_batch_to_current()` to force a batch
   re-sync at the next `EndOfTrack`.

**Why check the queue list, not `currently_playing`?**

- User-queued tracks (via `AddPlayableToQueue`) push batch tracks **back** in
  the Spotify queue but don't **remove** them. So `queue.queue` still contains
  our expected next track — just further down the list. Checking
  `currently_playing` would false-positive when a user-queued track is playing.
- The queue list is the most reliable view of what Spotify will play next.

**Cooldown**: Skip the consistency check for a few seconds after a batch
transition (`StartPlayback`), since Spotify's queue API takes time to reflect
the new batch. Use a `last_batch_transition: Option<Instant>` field on
`CustomQueue` to track this.

**On mismatch**: `truncate_batch_to_current()` is the safest response — it lets
the current track finish, then the next batch will be freshly computed and sent,
re-synchronizing with Spotify.

#### Config surface

```toml
# app.toml
[app_config]
# Enable app-managed queue for full playlist playback.
# Requires streaming. When disabled, playback uses Spotify-native queue management.
# Default: true
custom_queue = true
```

No runtime toggle command is needed for MVP. The config is read at startup.
Future enhancement: a `ToggleCustomQueue` command that flips the state mid-session
and clears/rebuilds the queue for the current playback.

---

### Implementation phases

The 10 steps have a dependency graph that groups naturally into 7 phases.
Each phase is a shippable increment — it compiles, passes clippy, and can be
tested in isolation. Phases are ordered so that earlier ones unlock the most
value with the least risk.

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4
  │              │                      │
  │              └──► Phase 5           └──► Phase 6
  │
  └──────────────────────────────────► Phase 7
```

#### Phase 1 — Foundation (Steps 1, 2, 10)

**Goal**: Scaffolding only. No behavioral change.

| Task | File(s)                               | What                                                                                                                                                                                                                                                   |
| ---- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1a   | `state/queue.rs` (new)                | `CustomQueue` struct, `ShuffleMode`, `AdvanceResult` enums. Basic methods: `new`, `current_batch`, `current_track`, `advance` (SameBatch / NewBatch / EndOfQueue only — no radio yet), `retreat`, `needs_new_batch`, `next_batch`, `remaining_tracks`. |
| 1b   | `state/mod.rs`                        | `mod queue; pub use queue::*;`                                                                                                                                                                                                                         |
| 2a   | `state/player.rs`                     | Add `custom_queue: Option<CustomQueue>` to `PlayerState`. Update `Default`.                                                                                                                                                                            |
| 2b   | `state/player.rs`                     | Update `playing_context_id()` to check `custom_queue.source_context` as fallback.                                                                                                                                                                      |
| 10a  | `config/mod.rs`                       | Add `custom_queue: bool` field to `AppConfig`, default `true`.                                                                                                                                                                                         |
| 10b  | `config/mod.rs` or `state/mod.rs`     | Helper `fn should_use_custom_queue(state: &SharedState) -> bool`.                                                                                                                                                                                      |
| 10c  | `docs/config.md`, `examples/app.toml` | Document the config option.                                                                                                                                                                                                                            |

**Issues resolved**: None yet.  
**Risk**: Zero — no codepaths change.  
**Effort**: Small.

---

#### Phase 2 — Core batching (Steps 3, 4-partial, 8)

**Goal**: Playlists play through all tracks via batched URIs. MVP.

| Task | File(s)                                 | What                                                                                                                                                                                                                                                         |
| ---- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 3a   | `event/window.rs`                       | In `handle_command_for_track_table_window`, when `should_use_custom_queue()` and context is Playlist/Album/Artist: build `Playback::URIs` from full `tracks` list; create `CustomQueue::new(…)` and include it in the `StartPlayback` request.               |
| 4a   | `client/mod.rs`                         | Add `state: &SharedState` param to `new_streaming_connection`. Pass `client_pub` clone into `streaming::new_connection`.                                                                                                                                     |
| 4b   | `streaming.rs`                          | Accept `client_pub: flume::Sender<ClientRequest>` in `new_connection`. In `EndOfTrack` handler: read `custom_queue`, if the ended track ID matches the batch's last track, call `advance()`. On `NewBatch` → send `StartPlayback(URIs(…))` via `client_pub`. |
| 4c   | `main.rs`                               | Thread `client_pub` through the `new_streaming_connection` call chain (currently called from `new_session` in `client/mod.rs` — need to store the sender in `AppClient`).                                                                                    |
| 8a   | `client/mod.rs`                         | In `handle_player_request` for `StartPlayback`: set `custom_queue` from the `Option<CustomQueue>` carried in the request (or clear if `None`).                                                                                                               |
| 8b   | `client/mod.rs`                         | In `handle_player_request` for `TransferPlayback`: clear `custom_queue`.                                                                                                                                                                                     |
| 8c   | `client/mod.rs`                         | In `retrieve_current_playback`: if context URI changed unexpectedly or playback disappeared, clear `custom_queue`.                                                                                                                                           |
| 8d   | `client/handlers.rs` or `client/mod.rs` | Queue consistency check: in the playback/queue polling loop, compare `play_order[position+1]` against `queue.queue`. On mismatch (with cooldown), call `truncate_batch_to_current()`. Add `last_batch_transition: Option<Instant>` to `CustomQueue`.         |

**Issues resolved**: **#766** (full playlist queueing), **#878** (sorted playback order).  
**Risk**: Medium — the `client_pub` plumbing through `streaming.rs` and `AppClient` is the most invasive change. The `EndOfTrack` batch-transition logic needs careful matching of track IDs.  
**Effort**: Medium-large. This is the biggest phase.

**Key design note for task 4c**: `new_streaming_connection` is called from
`new_session`, which doesn't have `client_pub`. Two options:

- **(A)** Store `client_pub: flume::Sender<ClientRequest>` inside `AppClient`.
  Simple, but couples the client to the channel.
- **(B)** Pass it through the call chain. More explicit, but requires changing
  `new_session`'s signature everywhere it's called.

**Choice: (A)** — store in `AppClient`. It already holds `Arc<Mutex<…>>` for
the stream connection. Adding a sender is consistent.

---

#### Phase 3 — Skip handling & repeat (Steps 5, 7)

**Goal**: NextTrack/PreviousTrack work correctly at batch boundaries. Repeat
wraps the playlist.

| Task | File(s)          | What                                                                                                                                                                    |
| ---- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5a   | `client/mod.rs`  | Add `state: &SharedState` param to `handle_player_request`. Update call site in `handle_request`.                                                                       |
| 5b   | `client/mod.rs`  | In `NextTrack`: if custom queue active and at batch end → start next batch. Else → `self.next_track()` as today.                                                        |
| 5c   | `client/mod.rs`  | In `PreviousTrack`: if at batch start → load previous batch. Else → `self.previous_track()` as today.                                                                   |
| 7a   | `client/mod.rs`  | In `Repeat` handler: also update `custom_queue.repeat`.                                                                                                                 |
| 7b   | `state/queue.rs` | `advance()` respects `repeat` state — wraps `position` and `batch_start` to 0 when `RepeatState::Context`. `RepeatState::Track` → don't advance (librespot handles it). |

**Issues resolved**: Completes #766 / #878 edge cases.  
**Risk**: Low — straightforward conditionals.  
**Effort**: Small.

---

#### Phase 4 — Shuffle (Step 6, partial)

**Goal**: Shuffle randomizes the full playlist, not just a ~100-track window.
Shuffle changes take effect at the next batch boundary without interrupting the
current song.

| Task | File(s)          | What                                                                                                                                                                                                                                                                                                   |
| ---- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 6a   | `state/queue.rs` | `set_shuffle_mode(ShuffleMode::Shuffle)`: Fisher-Yates permutation of `play_order`, keeping current track at position 0. Then call `truncate_batch_to_current()` so the change takes effect at the next `EndOfTrack` without restarting the current song.                                              |
| 6b   | `state/queue.rs` | `set_shuffle_mode(ShuffleMode::Off)`: Restore `play_order` to `original_tracks` order. Find current track's position in original order. Call `truncate_batch_to_current()`.                                                                                                                            |
| 6c   | `client/mod.rs`  | In `Shuffle` handler: if custom queue active, cycle Off↔Shuffle (two-state for now). Call `set_shuffle_mode`. **Do NOT send `StartPlayback`** — the current song continues playing. The next batch will use the new permuted order when `EndOfTrack` fires for the (now-truncated) batch's last track. |

**Issues resolved**: **#378** (automix / true random from full playlist).  
**Risk**: Low — shuffle is self-contained in the queue struct. The truncate approach avoids the complexity of mid-song playback restarts.  
**Effort**: Small-medium.

---

#### Phase 5 — Autoplay radio continuation (Step 4 extension)

**Goal**: Single-track playback continues with radio recommendations.

| Task | File(s)             | What                                                                                                                                                                                         |
| ---- | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5-1a | `state/queue.rs`    | Add `NeedsRadioTracks` variant to `AdvanceResult`. `advance()` returns it when at end of queue + `autoplay: true` + `!radio_tracks_appended`.                                                |
| 5-1b | `client/request.rs` | Add `ClientRequest::FetchAndContinueRadio` variant.                                                                                                                                          |
| 5-1c | `client/mod.rs`     | Handle `FetchAndContinueRadio`: read seed URI from `custom_queue.source_context` (or current track URI), call `radio_tracks()`, call `custom_queue.append_radio_tracks()`, start next batch. |
| 5-1d | `streaming.rs`      | In `EndOfTrack` handler: on `NeedsRadioTracks`, send `FetchAndContinueRadio`.                                                                                                                |
| 5-1e | `event/window.rs`   | In `handle_command_for_track_list_window`: when `should_use_custom_queue()` and `autoplay: true`, create a 1-track custom queue for search-result playback.                                  |

**Issues resolved**: **#767** (autoplay for single tracks).  
**Risk**: Medium — async radio fetch can fail; need graceful fallback (stop playback, log error). Also need to handle the case where the user has `autoplay: false` in device config.  
**Effort**: Medium.

---

#### Phase 6 — Smart shuffle (Step 6, full)

**Goal**: Three-state shuffle cycle with radio interleaving.

| Task | File(s)             | What                                                                                                                                                                                           |
| ---- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 6d   | `state/queue.rs`    | `set_shuffle_mode(ShuffleMode::SmartShuffle, radio_tracks)`: shuffle + interleave one radio track every 3–5 songs into `play_order`.                                                           |
| 6e   | `client/mod.rs`     | In `Shuffle` handler: extend cycle to Off → Shuffle → SmartShuffle → Off. On SmartShuffle transition, fetch radio tracks (async — may need to send a `ClientRequest` and handle the response). |
| 6f   | `client/request.rs` | Add `ClientRequest::SetSmartShuffle` if async fetch is needed (alternative: fetch inline in the handler since `handle_player_request` is already async).                                       |

**Issues resolved**: **#153** (enhance / smart shuffle), **#739** (shuffle toggle).  
**Risk**: Medium — radio fetch latency means a brief delay when switching to SmartShuffle. Need to handle the transition gracefully (show a loading state or optimistically switch and backfill).  
**Effort**: Medium.

---

#### Phase 7 — Queue UI (Step 9)

**Goal**: Queue page shows the full custom queue instead of Spotify's limited view.

| Task | File(s)          | What                                                                                                                                                                                                                                                                                   |
| ---- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 9a   | `ui/page.rs`     | In `render_queue_page`: when `custom_queue` is active, render `remaining_tracks()` instead of `player.queue.queue`. Need to resolve track metadata (the custom queue stores `PlayableId`s, not full track objects) — look up from `data.caches` or store `Track` structs in the queue. |
| 9b   | `ui/page.rs`     | For smart-shuffle mode: apply a distinct style (e.g., dimmed or different color) to interleaved recommendation tracks.                                                                                                                                                                 |
| 9c   | `state/queue.rs` | If metadata is needed: change `play_order` / `original_tracks` to store `Track` structs instead of bare `PlayableId`s, or add a parallel `track_metadata: Vec<Track>` field.                                                                                                           |

**Issues resolved**: UI completeness for all issues.  
**Risk**: Low — UI-only. The metadata question (9c) needs a decision but doesn't affect correctness.  
**Effort**: Small-medium.

---

### Summary timeline

| Phase                  | Steps           | Issues resolved | Depends on | Effort       |
| ---------------------- | --------------- | --------------- | ---------- | ------------ |
| **1 — Foundation**     | 1, 2, 10        | —               | —          | Small        |
| **2 — Core batching**  | 3, 4-partial, 8 | #766, #878      | Phase 1    | Medium-large |
| **3 — Skip & repeat**  | 5, 7            | (edge cases)    | Phase 2    | Small        |
| **4 — Shuffle**        | 6-partial       | #378            | Phase 2    | Small-medium |
| **5 — Autoplay radio** | 4-extension     | #767            | Phase 2    | Medium       |
| **6 — Smart shuffle**  | 6-full          | #153, #739      | Phase 4, 5 | Medium       |
| **7 — Queue UI**       | 9               | (UI)            | Phase 1    | Small-medium |

**Critical path**: Phase 1 → Phase 2 → Phase 3. This is the minimum to ship a
useful improvement. Phases 4–7 can be done in any order after Phase 2, with
Phase 6 depending on both 4 and 5.

**Suggested milestone plan**:

- **Milestone 1** (Phases 1–3): Core custom queue. Ship as a PR. Fixes #766, #878.
- **Milestone 2** (Phases 4–5): Shuffle + autoplay. Ship as a follow-up PR. Fixes #378, #767.
- **Milestone 3** (Phases 6–7): Smart shuffle + queue UI. Ship as final PR. Fixes #153, #739.

---

### Steps

#### 1. Add `CustomQueue` struct

New file: `spotify_player/src/state/queue.rs`

```rust
/// Shuffle mode for the custom queue
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ShuffleMode {
    #[default]
    Off,
    /// Standard shuffle — randomize the full track order
    Shuffle,
    /// Smart shuffle — shuffle + interleave radio recommendations
    SmartShuffle,
}

/// App-managed playback queue that replaces spirc-managed queueing.
#[derive(Clone, Debug)]
pub struct CustomQueue {
    /// Original ordered track list (from the context, respecting client-side sort)
    pub original_tracks: Vec<PlayableId<'static>>,
    /// The effective play order — indices into `original_tracks`.
    /// When shuffle is off this is `[0, 1, 2, …]`; when on it's a permutation.
    /// When smart-shuffle is on, extra recommendation track IDs are interleaved
    /// (stored directly, not as indices).
    pub play_order: Vec<PlayableId<'static>>,
    /// Current position within `play_order`
    pub position: usize,
    /// Start index of the current batch within `play_order`
    pub batch_start: usize,
    /// Exclusive end index of the current batch within `play_order`.
    /// Tracks `play_order[batch_start..batch_end]` are the current batch.
    /// Normally `batch_end = min(batch_start + max_batch_size, play_order.len())`,
    /// but `truncate_batch_to_current()` can shrink it to `position + 1`.
    pub batch_end: usize,
    /// Maximum tracks per Spotify API batch (= `tracks_playback_limit`)
    pub max_batch_size: usize,
    /// Original context (for "playing from" display and radio seed)
    pub source_context: Option<ContextId>,
    /// Local repeat state mirroring the player's repeat
    pub repeat: rspotify::model::RepeatState,
    /// Current shuffle mode (Off / Shuffle / SmartShuffle)
    pub shuffle_mode: ShuffleMode,
    /// Whether radio tracks have been fetched for autoplay/smart-shuffle
    pub radio_tracks_appended: bool,
    /// Timestamp of last batch transition, used for consistency check cooldown.
    /// Skip queue consistency checks for a few seconds after sending a new batch,
    /// since Spotify's queue API takes time to reflect the change.
    pub last_batch_transition: Option<std::time::Instant>,
}
```

Key methods:

- `new(tracks, start_position, max_batch_size, context)` — build queue; `play_order` = identity;
  `batch_end = min(batch_start + max_batch_size, play_order.len())`
- `current_batch() -> Vec<PlayableId>` — `play_order[batch_start..batch_end]`
- `current_track() -> &PlayableId` — `play_order[position]`
- `advance() -> AdvanceResult` — move forward; returns `SameBatch`, `NewBatch(Vec<PlayableId>)`,
  `EndOfQueue`, or `NeedsRadioTracks`
- `retreat() -> RetreatResult` — move backward; analogous
- `needs_new_batch() -> bool` — true when `position >= batch_end`
- `next_batch() -> Option<Vec<PlayableId>>` — compute next batch: `batch_start = batch_end`,
  `batch_end = min(batch_start + max_batch_size, play_order.len())`
- `truncate_batch_to_current()` — set `batch_end = position + 1`, so the current track becomes
  the last track in the batch. The next `EndOfTrack` event will trigger a batch transition with
  the new state (shuffle permutation, repeat mode, etc.) **without interrupting the current song**.
  This is the key mechanism for non-interrupting shuffle/repeat changes.
- `set_shuffle_mode(mode, radio_tracks: Option<Vec<PlayableId>>)` — Off: restore `original_tracks`
  order; Shuffle: Fisher-Yates permutation keeping current track at front; SmartShuffle: shuffle +
  interleave `radio_tracks` every N songs. After permuting, calls `truncate_batch_to_current()` so
  the change takes effect at the next batch boundary without restarting the current track.
- `append_radio_tracks(tracks: Vec<PlayableId>)` — extend `play_order` with radio tracks for autoplay
- `remaining_tracks() -> &[PlayableId]` — `play_order[position+1..]` for queue UI display

Wire into `state/mod.rs`: `mod queue; pub use queue::*;`

#### 2. Add `custom_queue` field to `PlayerState`

In [state/player.rs](spotify_player/src/state/player.rs):

```rust
pub custom_queue: Option<CustomQueue>,
```

Update `PlayerState::playing_context_id()`: when `playback.context` is `None` (URIs playback),
fall back to `custom_queue.as_ref().and_then(|q| q.source_context.clone())` before checking
`currently_playing_tracks_id`. This makes the "playing from [playlist]" page work correctly.

#### 3. Populate custom queue on `StartPlayback` for track-table contexts

In [event/window.rs](spotify_player/src/event/window.rs#L307-L340)
`handle_command_for_track_table_window`, within the `PlayRandom | ChooseSelected` arm:

- When `context_id` is `Some(ContextId::Playlist(_) | ContextId::Album(_) | ContextId::Artist(_))`
  **and** `state.is_streaming_enabled()`, always build a `Playback::URIs` from the **full `tracks`
  list** (which already reflects any client-side sort/reverse) instead of `Playback::Context`.
- Create a `CustomQueue::new(…)` from that list and store it:
  `state.player.write().custom_queue = Some(queue);`
- The initial batch is still `uri_offset(uri, tracks_playback_limit)` as today.

**This resolves #878** (sorted order) because the `tracks` slice is read _after_ the sort
commands mutate `data.context_tracks_mut()`, so the custom queue captures the
user-visible order.

**This resolves #766** (full queueing) because the custom queue holds _all_ tracks, not just
the ~100 of the spirc window.

For non-streaming mode, keep the existing `Playback::Context` path unchanged.

#### 4. Detect batch exhaustion and autoplay in streaming event handler

In [streaming.rs](spotify_player/src/streaming.rs), the `PlayerEvent` task currently handles
`Playing`, `Paused`, `Changed`, and `EndOfTrack`. Extend the `EndOfTrack` handler:

```
EndOfTrack { playable_id } => {
    let mut player = state.player.write();
    if let Some(ref mut queue) = player.custom_queue {
        match queue.advance() {
            AdvanceResult::SameBatch => { /* librespot handles it */ }
            AdvanceResult::NewBatch(batch) => {
                client_pub.send(StartPlayback(URIs(batch, None), None));
            }
            AdvanceResult::NeedsRadioTracks => {
                // Fetch radio tracks asynchronously, then append & start next batch
                client_pub.send(ClientRequest::FetchAndContinueRadio);
            }
            AdvanceResult::EndOfQueue => { /* playback stops */ }
        }
    }
}
```

**This resolves #767** (autoplay for single tracks): when the queue has a single track
and `advance()` returns `NeedsRadioTracks`, the handler fetches radio recommendations
using `radio_tracks(seed_uri)` and appends them to the queue, continuing playback.

Requires passing `client_pub: flume::Sender<ClientRequest>` into `streaming::new_connection()`.
Update `new_streaming_connection` in [client/mod.rs](spotify_player/src/client/mod.rs#L192-L195)
and [main.rs](spotify_player/src/main.rs) to thread it through.

Also add a new `ClientRequest::FetchAndContinueRadio` variant that:

1. Takes the current queue's seed context URI
2. Calls `self.radio_tracks(seed_uri)`
3. Appends the results to `custom_queue.play_order` via `append_radio_tracks()`
4. Starts the next batch

#### 5. Override `NextTrack` / `PreviousTrack` at batch boundaries

In [client/mod.rs](spotify_player/src/client/mod.rs#L286-L288)
`handle_player_request`:

Following the **batch-manager model**, `NextTrack` / `PreviousTrack` only
intervene when the skip would cross a batch boundary:

- For `NextTrack`: check if custom queue is active and `position` is at the
  last track of the current batch.
  - **Within batch** → call `self.next_track(device_id)` as usual. Do NOT
    update the custom queue position — let `EndOfTrack` be the sole
    position-tracking mechanism (avoids user-queued track desync).
  - **At batch boundary** → compute next batch, call
    `self.start_playback(URIs(next_batch, None), device_id)`.
  - **End of queue + autoplay** → send `FetchAndContinueRadio`.
  - **End of queue, no autoplay** → do nothing.
- Similarly for `PreviousTrack`: at the first track of a batch, load the
  previous batch.

Add `state: &SharedState` parameter to `handle_player_request`, updating the call
site in `handle_request` ([client/mod.rs#L405](spotify_player/src/client/mod.rs#L405)).

User-queued tracks (via `AddPlayableToQueue`) are handled transparently:
they live in the native Spotify queue and play via `next_track()` without
the custom queue needing to know about them.

#### 6. Three-state Shuffle: Off → Shuffle → Smart Shuffle → Off

In the `PlayerRequest::Shuffle` handler
([client/mod.rs#L330-L334](spotify_player/src/client/mod.rs#L330-L334)):

When custom queue is active, cycle through `ShuffleMode`:

- **Off → Shuffle**: Fisher-Yates permutation of `play_order`, keeping current track at front.
  Call `truncate_batch_to_current()` so the current song keeps playing uninterrupted.
  Call `self.shuffle(true, device_id)` to sync Spotify's shuffle flag.
  The new permuted order takes effect when `EndOfTrack` fires for the current track (which is now
  the batch's last track due to truncation), triggering a new batch from the shuffled order.
- **Shuffle → SmartShuffle**: Fetch radio tracks for the source context
  (`radio_tracks(source_context.uri())`), interleave them into `play_order` (e.g., one
  recommendation every 3–5 songs). Call `truncate_batch_to_current()`.
  Keep `self.shuffle(true, device_id)`.
- **SmartShuffle → Off**: Restore `play_order` to `original_tracks` order, remove interleaved
  radio tracks. Find current track's original position. Call `truncate_batch_to_current()`.
  Call `self.shuffle(false, device_id)`.

**This resolves #739** (smart shuffle toggle) and **#153** (enhance / smart shuffle).

When custom queue is _not_ active, keep the existing two-state toggle.

#### 7. Override `Repeat` when custom queue is active

In the `PlayerRequest::Repeat` handler
([client/mod.rs#L316-L326](spotify_player/src/client/mod.rs#L316-L326)):

- Keep the Spotify API call.
- Also update `custom_queue.repeat`.
- `advance()` uses this to decide whether to wrap at playlist end.

#### 8. Clear custom queue on transitions

As detailed in the **Optionality and transition design** section:

- **`StartPlayback`**: In `handle_player_request`, set `custom_queue` from the
  `Option<CustomQueue>` carried in the `StartPlayback` request variant (or clear
  it if `None`). The custom queue travels with the request — no intermediate
  staging field is needed. This avoids race windows where a stale `EndOfTrack`
  could fire between writing a pending field and the handler promoting it.
- **`TransferPlayback`**: Always clear `custom_queue`. External devices manage
  their own queue.
- **`retrieve_current_playback`**: If the Spotify-reported context URI changes
  unexpectedly (not matching `custom_queue.source_context`), clear the queue.
  This handles external context changes from other devices/apps.
- **Playback disappears**: If `playback` becomes `None`, clear the queue.

For single-track playback from search with `autoplay: true`, a 1-track
custom queue is created so the `EndOfTrack` handler can fetch radio tracks.
With `autoplay: false`, no queue is created and playback stops after the track.

#### 9. Update queue UI to reflect custom queue

In [ui/page.rs](spotify_player/src/ui/page.rs#L719-L806) `render_queue_page`:

- When `custom_queue` is active, render `custom_queue.remaining_tracks()` instead of
  `player.queue.queue`. This shows the full upcoming track list.
- For smart-shuffle mode, visually distinguish interleaved recommendation tracks
  (e.g., different color or a marker icon).

#### 10. Add config option `custom_queue` (default `true`)

In [config/mod.rs](spotify_player/src/config/mod.rs#L51) `AppConfig`:

- `pub custom_queue: bool` — default `true`.
- Gate all custom queue activation behind:
  ```rust
  state.is_streaming_enabled() && config::get_config().app_config.custom_queue
  ```
  Extract this into a helper: `fn should_use_custom_queue(state: &SharedState) -> bool`.
- When `false`, the app behaves identically to the current codebase — no custom
  queue is ever created, all playback uses native Spotify/librespot queue
  management. This is a clean opt-out with zero behavioral change.
- Document in [docs/config.md](docs/config.md) and [examples/app.toml](examples/app.toml).

---

### Verification

- **#766 — Large playlist**: Play track #1 of a 200+ track playlist. Skip through. Verify playback
  continues past the ~100-track boundary. Queue page shows all remaining tracks.
- **#878 — Sorted playback**: Sort a playlist by recently added, reverse. Play first track. Verify
  the next track matches the visible sorted order.
- **#378 — Automix / true random**: Enable shuffle on a 200+ track playlist. Verify tracks come from
  the full list, not just a ~100-track window. Let it play through — verify no looping on a subset.
- **#767 — Single-track autoplay**: Search for a song, play it. Verify that after it ends,
  related/radio tracks start playing automatically.
- **#153 / #739 — Smart shuffle**: On a playlist, press Shuffle twice (Off → Shuffle → Smart Shuffle).
  Verify recommendation tracks are interleaved. Press again to return to Off.
- **Batch boundary**: Skip to the last track of a batch, press next — verify seamless transition.
- **Repeat**: With repeat on, verify playlist wraps after the last track.
- **Non-streaming**: Verify context playback still works normally on external devices.
- **User-queued tracks**: While custom queue is active, add a track to queue. Verify it plays
  after the current track, then the custom queue resumes normally.
- **Device transfer out**: While custom queue is active, transfer to phone. Verify playback
  continues on phone; custom queue clears; native queue takes over.
- **Device transfer back**: Transfer back to integrated player. Verify no stale custom queue.
  Start a new playlist — verify custom queue rebuilds.
- **Config opt-out**: Set `custom_queue = false`. Verify all playback uses native Spotify behavior.
  Large playlists should exhibit the old ~100-track limitation.
- **Shuffle non-interrupting**: While a song is playing, toggle shuffle. Verify the current song
  continues playing without restart. After it finishes, verify the next song comes from the
  shuffled (or unshuffled) order.
- **State reconciliation — external context change**: While custom queue is active, change the
  playlist on an external device/app. Verify the custom queue clears and the app picks up the
  new external playback context.
- **State reconciliation — playback disappears**: While custom queue is active, stop playback
  completely (e.g., disconnect device). Verify custom queue clears.
- **Queue consistency check**: Simulate a desync (e.g., external queue manipulation). Verify the
  consistency check detects mismatch and triggers `truncate_batch_to_current()`, re-syncing at
  the next batch boundary.
- **Queue consistency — user-queued track**: Add a track to queue while custom queue is active.
  Verify the consistency check does NOT false-positive (the expected next batch track should
  still be in the Spotify queue, just pushed back).
- **Clippy**: `cargo clippy --features streaming,rodio-backend,media-control,image,notify,daemon,fzf -- -D warnings`
  and `cargo clippy --no-default-features`.
- **Format**: `cargo fmt --all`.

### Decisions

- **URIs batching over context playback**: The Spotify API limits context-based queue windows to ~100
  tracks. Trade-off: "Playing from [playlist]" won't show natively on other devices, but the custom
  queue's `source_context` preserves this for the TUI.
- **Streaming-only activation**: Custom queue only activates when `is_streaming_enabled()` is true.
  External devices manage their own queue and context playback works better there.
- **Batch-manager model (not per-track tracker)**: The custom queue only intervenes at batch
  boundaries. Within a batch, librespot handles next/previous natively. This avoids race
  conditions with Spotify's eventual consistency and keeps user-queued tracks working
  transparently.
- **EndOfTrack as sole position tracker**: `NextTrack`/`PreviousTrack` do NOT advance the custom
  queue position — they delegate to the Spotify API and let `EndOfTrack` events be the source of
  truth. This avoids desync when user-queued tracks are in the native queue.
- **Batch size = `tracks_playback_limit`**: Reuses existing config for simplicity.
- **Three-state shuffle cycle**: Rather than a separate keybinding for smart shuffle (#739), cycling
  through three states on the existing `Shuffle` command is minimal UI surface and matches the
  official Spotify client behavior.
- **Radio-track fetching for autoplay**: Reuses the existing `radio_tracks()` Mercury API, which
  already works for the radio context. This avoids introducing new API dependencies.
- **`custom_queue` config**: Clean opt-out with zero behavioral change. Default `true` because
  the custom queue is strictly an improvement for streaming users.
- **No persistence across restarts**: Custom queue is in-memory only. On restart the app picks up
  the current Spotify playback state without a queue. Acceptable for MVP.
- **`batch_end` instead of fixed `batch_size`**: Using a mutable `batch_end` index (instead of
  always computing batch end as `batch_start + batch_size`) allows `truncate_batch_to_current()`
  to shrink the current batch dynamically. This enables non-interrupting shuffle/repeat changes.
- **`truncate_batch_to_current()` for non-interrupting state changes**: When shuffle mode or
  repeat state changes mid-song, instead of restarting playback with a new batch (which would
  cut off the current track), we truncate the batch so the current track is its last entry.
  The next `EndOfTrack` then triggers a batch transition with the new state applied. The user
  hears zero interruption.
- **State reconciliation: custom queue is write-only**: The custom queue never reads Spotify API
  state to update its own position. `EndOfTrack` is the sole position tracker. The Spotify poll
  only clears the queue at three junction points (context mismatch, playback disappears, device
  transfer). This avoids race conditions with Spotify's eventual consistency.
- **Queue-based consistency checking over `currently_playing`**: Checking whether the expected
  next track appears in Spotify's `queue.queue` list is more robust than checking
  `currently_playing`, because user-queued tracks push batch tracks back in the queue but don't
  remove them. This avoids false positives when user-queued tracks are playing.
