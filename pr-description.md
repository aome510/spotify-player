## Summary
- Replace `viuer` with `ratatui-image` for rendering cover images
- Remove manual `set_skip` / `clear_area` hacks — `ratatui-image` renders as a native ratatui widget
- Remove `viuer` initialization in `main.rs`, use `Picker::from_query_stdio()` for protocol auto-detection
- Remove `cover_img_scale` usage (no longer needed — scaling is handled by `ratatui-image`)

## Notes
`Picker::from_query_stdio()` must run after `enable_raw_mode()` but before `EnterAlternateScreen` for correct protocol detection.

In nested terminals (e.g. neovim floating terminal), stdio queries don't reach the actual terminal emulator, so the protocol falls back to Halfblocks. This is a known limitation of `ratatui-image`'s detection approach. Regular terminals (foot, kitty, iTerm2, etc.) work correctly.

README still references `viuer` — leaving that for maintainer to decide how to update.
