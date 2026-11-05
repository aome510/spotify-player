## Phase 2 — Core batching: Implementation sub-steps

Phase 2 is the biggest phase. It breaks down into 4 small, independently
compilable changes with a clear dependency order:

```
2.1 → 2.2 → 2.3
           → 2.4
```

Sub-steps 2.3 and 2.4 are independent of each other, both depending on 2.2.

### Design decision: no changes to `PlayerRequest::StartPlayback`

The custom queue is set directly on `PlayerState` from the event thread
**before** sending `StartPlayback`, rather than bundling it inside the request.
This is simpler because:

- `StartPlayback` is used everywhere (CLI, popups, media controls) — adding an
  extra field would require touching every call site for no reason.
- Batch continuations from `EndOfTrack` also send `StartPlayback` — they must
  NOT clear or replace the existing queue, which would require awkward
  "don't clear if already set" logic in the handler.
- `CustomQueue` is large (full track list). Cloning it into a channel message
  is wasteful.
- The `flume` channel guarantees ordering: the event thread's write to
  `custom_queue` happens-before the async handler processes `StartPlayback`.

---

### Sub-step 2.1 — Build `CustomQueue` in the event thread

**Goal**: When the user starts playback from a track table, create a
`CustomQueue` and set it on `PlayerState` before sending `StartPlayback`.

**Files**: `event/window.rs`

**Changes**:

In `handle_command_for_track_table_window`, inside the
`Command::PlayRandom | Command::ChooseSelected` arm (L308):

1. After determining `uri`, check `state.should_use_custom_queue()` **and**
   `context_id` matches `Some(ContextId::Playlist(_) | ContextId::Album(_) | ContextId::Artist(_))`.

2. When the custom queue should be used:
   - Always build `Playback::URIs` from the full `tracks` list (not
     `Playback::Context`). This ensures the app controls the play order.
   - Determine `start_position`: the index of the selected track in `tracks`.
   - Create `CustomQueue::new(track_ids, start_position, tracks_playback_limit, Some(context_id), autoplay)`.
     - `track_ids`: `tracks.iter().map(|t| t.id.clone().into()).collect()`
     - `autoplay`: default to `false` for now (Phase 5 wires up the device config)
   - Set `state.player.write().custom_queue = Some(queue)` **before** sending
     the `StartPlayback` request.
   - Still call `.uri_offset(uri, tracks_playback_limit)` on the playback so
     the first API call sends only a batch-sized window (same as today).

3. When the custom queue should NOT be used (streaming disabled, config off,
   show context): keep existing behavior. Clear any stale queue:
   `state.player.write().custom_queue = None`.

**Key insight**: The `tracks` slice in `handle_command_for_track_table_window`
already reflects any client-side sort/reverse. Building the `CustomQueue` from
it captures the user-visible order — this is what fixes #878.

**Verification**: `cargo clippy`. Manual test: start a playlist → inspect
`state.player.read().custom_queue` (via debug logging). The queue should be
`Some(...)` with the correct track list. Playback of the first batch works
exactly as before.

---

### Sub-step 2.2 — Thread `client_pub` into streaming; handle `EndOfTrack` for batch transitions

**Goal**: When a batch ends, the streaming handler detects it and sends the
next batch. This is the critical change that makes large playlists play through.

**Files**: `client/mod.rs`, `streaming.rs`, `main.rs`

**Changes**:

#### A. Store `client_pub` in `AppClient`

1. Add `client_pub: flume::Sender<ClientRequest>` field to `AppClient`.
2. Set it during `AppClient` construction (in `main.rs` — the
   `flume::unbounded()` channel already exists there).

#### B. Pass `client_pub` into streaming

3. In `new_streaming_connection` (L232): pass `self.client_pub.clone()` to
   `streaming::new_connection`.
4. Update `streaming::new_connection` signature to accept
   `client_pub: flume::Sender<ClientRequest>`.

#### C. Handle `EndOfTrack` batch transitions

5. In the player event task (streaming.rs L218), extend the match on
   `PlayerEvent` to handle `EndOfTrack`:

   ```rust
   PlayerEvent::EndOfTrack { .. } => {
       let mut player = state.player.write();
       if let Some(ref mut queue) = player.custom_queue {
           match queue.advance() {
               AdvanceResult::SameBatch => { /* librespot handles it */ }
               AdvanceResult::NewBatch(batch) => {
                   let playback = Playback::URIs(batch, None);
                   // Drop the write lock before sending to avoid deadlock
                   drop(player);
                   let _ = client_pub.send(ClientRequest::Player(
                       PlayerRequest::StartPlayback(playback, None),
                   ));
               }
               AdvanceResult::NeedsRadioTracks => {
                   // Phase 5 — for now, playback stops
               }
               AdvanceResult::EndOfQueue => {
                   // Playback stops naturally
               }
           }
       }
   }
   ```

   Note: The `StartPlayback` sent from streaming does NOT modify
   `custom_queue` — the queue is already in place on `PlayerState`. The async
   handler just sends the batch to Spotify as a URIs playback.

**Verification**: `cargo clippy`. Manual test: play a playlist with >50 tracks
→ when the first batch ends, the next batch auto-starts seamlessly. The
"playing from" context is preserved.

---

### Sub-step 2.3 — Clear `custom_queue` on transitions

**Goal**: Ensure the custom queue is properly cleaned up when playback leaves
the queue's context.

**Files**: `client/mod.rs`

**Changes**:

1. `handle_player_request` needs `state: &SharedState` — update the signature
   and the call site in `handle_request` (L402 — `state` is already available).

2. In `handle_player_request` for `TransferPlayback` (L259):
   ```rust
   state.player.write().custom_queue = None;
   ```

3. In `retrieve_current_playback` (L1598), after updating `player.playback`:
   - If `player.playback` is `None` → `player.custom_queue = None`
   - If `player.custom_queue` is `Some` and the Spotify-reported context is
     **non-None** with a URI that differs from
     `queue.source_context().map(|c| c.uri())` → `player.custom_queue = None`
   - If Spotify reports `context: None` → that's expected for URIs playback,
     don't clear.

**Verification**: `cargo clippy`. Manual test:
- Transfer playback to phone → queue is cleared
- Play from another device/app → queue is cleared
- Normal batch playback continues undisturbed

---

### Sub-step 2.4 — Queue consistency check

**Goal**: Safety net that detects silent divergence between the custom queue and
Spotify's actual queue state.

**Files**: `client/mod.rs` (near `GetCurrentUserQueue` handling)

**Changes**:

1. In the polling section that handles `GetCurrentUserQueue` (or in
   `retrieve_current_playback`), when both `custom_queue` and `player.queue`
   are available:

   ```rust
   if let Some(ref mut queue) = player.custom_queue {
       // Skip check during cooldown after batch transition
       let in_cooldown = queue.last_batch_transition()
           .is_some_and(|t| t.elapsed() < Duration::from_secs(3));
       
       if !in_cooldown {
           if let Some(expected) = queue.expected_next_track() {
               let found = player.queue.queue.iter().any(|item| {
                   match item {
                       PlayableItem::Track(t) => t.id.as_ref()
                           .is_some_and(|id| id.uri() == expected.uri()),
                       PlayableItem::Episode(e) => e.id.uri() == expected.uri(),
                       _ => false,
                   }
               });
               if !found {
                   tracing::warn!(
                       "Custom queue consistency check failed: \
                        expected next track not found in Spotify queue. \
                        Forcing re-sync."
                   );
                   queue.truncate_batch_to_current();
               }
           }
       }
   }
   ```

2. This is a read-then-mutate on `custom_queue` — needs a write lock on
   `player` state.

**Verification**: `cargo clippy`. This is a safety net — difficult to test
manually but ensures resilience against edge cases.

---

### Summary

| Sub-step | Description                          | Risk   | Effort |
| -------- | ------------------------------------ | ------ | ------ |
| 2.1      | Build queue in event thread          | Low    | Small  |
| 2.2      | Streaming `EndOfTrack` batch handler | Medium | Medium |
| 2.3      | Clear queue on transitions           | Low    | Small  |
| 2.4      | Consistency check safety net         | Low    | Small  |

After all 4 sub-steps, a large playlist plays through all tracks seamlessly via
batched URIs. This resolves **#766** (full playlist queueing) and **#878**
(sorted playback order).
