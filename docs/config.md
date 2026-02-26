# Configuration Documentation

## Table of Contents

- [General](#general)
  - [Notes](#notes)
  - [Media control](#media-control)
  - [Player event hook command](#player-event-hook-command)
  - [Client id command](#client-id-command)
  - [Device configurations](#device-configurations)
  - [Layout configurations](#layout-configurations)
- [Themes](#themes)
  - [Use script to add theme](#use-script-to-add-theme)
  - [Palette](#palette)
  - [Component Styles](#component-styles)
- [Keymaps](#keymaps)

Configuration files are located in the application's configuration directory, which defaults to `$HOME/.config/spotify-player`.

## General

A sample `app.toml` is available at [examples/app.toml](../examples/app.toml).

`spotify_player` uses `app.toml` for application settings. Available options:

| Option                            | Description                                                                                    | Default                                                                |
| --------------------------------- | ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `client_id`                       | Spotify client ID for API access. Uses a default if not specified.                             | See code (default: ncspot's client ID)                                 |
| `client_id_command`               | Shell command that outputs client ID to stdout (overrides `client_id`).                        | `None`                                                                 |
| `login_redirect_uri`              | Redirect URI for authentication.                                                               | `http://127.0.0.1:8989/login`                                          |
| `client_port`                     | Port for the application's client to handle CLI commands.                                      | `8080`                                                                 |
| `log_folder`                      | Path to store log files.                                                                       | `None`                                                                 |
| `tracks_playback_limit`           | Maximum number of tracks in a playback session.                                                | `50`                                                                   |
| `playback_format`                 | Format string for the playback window.                                                         | `{status} {track} • {artists} {liked}\n{album} • {genres}\n{metadata}` |
| `playback_metadata_fields`        | Ordered list of metadata fields displayed in the playback UI `{metadata}` placeholder.         | `["repeat", "shuffle", "volume", "device"]`                            |
| `notify_format`                   | Notification format (if `notify` feature enabled).                                             | `{ summary = "{track} • {artists}", body = "{album}" }`                |
| `notify_timeout_in_secs`          | Notification timeout in seconds (if `notify` feature enabled).                                 | `0`                                                                    |
| `notify_transient`                | Send transient notifications (Linux only, if `notify` feature enabled).                        | `false`                                                                |
| `player_event_hook_command`       | Command to execute on player events.                                                           | `None`                                                                 |
| `ap_port`                         | Spotify session connection port.                                                               | `None`                                                                 |
| `proxy`                           | Spotify session connection proxy.                                                              | `None`                                                                 |
| `theme`                           | Name of the theme to use.                                                                      | `default`                                                              |
| `app_refresh_duration_in_ms`      | Interval (ms) between application refreshes.                                                   | `32`                                                                   |
| `playback_refresh_duration_in_ms` | Interval (ms) between playback refreshes.                                                      | `0`                                                                    |
| `page_size_in_rows`               | Number of rows per page for navigation.                                                        | `20`                                                                   |
| `enable_media_control`            | Enable media control support (requires `media-control` feature).                               | `true` (Linux), `false` (macOS/Windows)                                |
| `enable_streaming`                | Enable streaming (`Always`, `Never`, or `DaemonOnly`).                                         | `Always`                                                               |
| `enable_notify`                   | Enable notifications (requires `notify` feature).                                              | `true`                                                                 |
| `enable_cover_image_cache`        | Cache album cover images.                                                                      | `true`                                                                 |
| `notify_streaming_only`           | Send notifications only when streaming is active (requires `streaming` and `notify` features). | `false`                                                                |
| `default_device`                  | Default device to connect to on startup.                                                       | `spotify-player`                                                       |
| `play_icon`                       | Icon for playing state.                                                                        | `▶`                                                                    |
| `pause_icon`                      | Icon for paused state.                                                                         | `▌▌`                                                                   |
| `liked_icon`                      | Icon for liked songs.                                                                          | `♥`                                                                    |
| `explicit_icon`                   | Icon for explicit songs.                                                                       | `(E)`                                                                  |
| `border_type`                     | Border style: `Hidden`, `Plain`, `Rounded`, `Double`, or `Thick`.                              | `Plain`                                                                |
| `progress_bar_type`               | Progress bar style: `Rectangle` or `Line`.                                                     | `Rectangle`                                                            |
| `progress_bar_position`           | Progress bar position: `Bottom` or `Right`.                                                    | `Bottom`                                                               |
| `layout`                          | Layout configuration (see below).                                                              | See below                                                              |
| `genre_num`                       | Max number of genres to display in playback text.                                              | `2`                                                                    |
| `cover_img_length`                | Cover image length (requires `image` feature).                                                 | `9`                                                                    |
| `cover_img_width`                 | Cover image width (requires `image` feature).                                                  | `5`                                                                    |
| `cover_img_scale`                 | Cover image scale (requires `image` feature).                                                  | `1.0`                                                                  |
| `cover_img_pixels`                | Pixels per side for cover image (requires `pixelate` feature).                                 | `16`                                                                   |
| `seek_duration_secs`              | Seek duration in seconds for seek commands.                                                    | `5`                                                                    |
| `sort_artist_albums_by_type`      | Sort albums by type on artist pages.                                                           | `false`                                                                |
| `volume_scroll_step`              | Volume change step when using mouse scroll.                                                    | `5`                                                                    |
| `enable_mouse_scroll_volume`      | Enable volume control via mouse scroll.                                                        | `true`                                                                 |
| `device`                          | Device configuration (see below).                                                              | See below                                                              |

### Notes

- By default, `spotify-player` uses [ncspot](https://github.com/hrkfdn/ncspot)'s client ID for compatibility with Spotify's API. See [this issue](https://github.com/aome510/spotify-player/issues/890) for details.
- `ap_port` and `proxy` are passed to Librespot for session configuration. Librespot uses its defaults if unset.
- Setting a positive `app_refresh_duration_in_ms` increases API usage and may trigger rate limits. By default, `playback_refresh_duration_in_ms=0` refreshes playback only on events or commands.
- `enable_streaming` accepts `Always`, `Never`, or `DaemonOnly`. For backward compatibility, `true`/`false` are also accepted.
- `border_type`, `progress_bar_type`, and `progress_bar_position` accept only the values listed in the table above.
- `explicit_icon` can be set to any Unicode character or an empty string to disable explicit markers.

#### Media control

Media control (`enable_media_control`) is enabled by default on Linux but disabled on macOS and Windows. On these platforms, the OS requires an open window to receive media events, which may cause the terminal to lose focus on startup.

### Player event hook command

`player_event_hook_command` is an object with `command` and `args` fields. On each player event, the command executes with the event data passed as arguments.

A player event is represented as a list of arguments with either of the following values:

- `"Changed" NEW_TRACK_ID`
- `"Playing" TRACK_ID POSITION_MS`
- `"Paused" TRACK_ID POSITION_MS`
- `"EndOfTrack" TRACK_ID`

**Note**: If `args` is specified, these arguments precede the event arguments.

For example, with `player_event_hook_command = { command = "a.sh", args = ["-b", "c", "-d"] }`, a `Changed` event with `NEW_TRACK_ID=id` executes:

```shell
a.sh -b c -d Changed id
```

Example script that reads event data from arguments and logs to a file:

```sh
#!/bin/bash

set -euo pipefail

case "$1" in
    "Changed") echo "command: $1, new_track_id: $2" >> /tmp/log.txt ;;
    "Playing") echo "command: $1, track_id: $2, position_ms: $3" >> /tmp/log.txt ;;
    "Paused") echo "command: $1, track_id: $2, position_ms: $3" >> /tmp/log.txt ;;
    "EndOfTrack") echo "command: $1, track_id: $2" >> /tmp/log.txt ;;
esac
```

### Client id command

To securely store your `client_id`, use `client_id_command` with a `command` and optional `args`. Example:

```toml
client_id_command = { command = "cat", args = ["/full/path/to/file"] }
```

**Note**: Use absolute paths; `~` is not expanded.

### Device configuration

Device options are configured in the `[device]` section:

| Option          | Description                              | Default          |
| --------------- | ---------------------------------------- | ---------------- |
| `name`          | Device name.                             | `spotify-player` |
| `device_type`   | Device type.                             | `speaker`        |
| `volume`        | Initial volume (percent).                | `70`             |
| `bitrate`       | Bitrate in kbps (`96`, `160`, or `320`). | `320`            |
| `audio_cache`   | Enable audio file caching.               | `false`          |
| `normalization` | Enable audio normalization.              | `false`          |
| `autoplay`      | Enable autoplay of similar songs.        | `false`          |

See the [Librespot wiki](https://github.com/librespot-org/librespot/wiki/Options) for more details on these options.

### Layout configuration

The `[layout]` section configures the UI layout:

| Option                     | Description                                          | Default |
| -------------------------- | ---------------------------------------------------- | ------- |
| `library.album_percent`    | Percentage of the album window in the library.       | `40`    |
| `library.playlist_percent` | Percentage of the playlist window in the library.    | `40`    |
| `playback_window_position` | Position of the playback window (`Top` or `Bottom`). | `Top`   |
| `playback_window_height`   | Height of the playback window.                       | `6`     |

Example:

```toml

[layout]
library = { album_percent = 40, playlist_percent = 40 }
playback_window_position = "Top"

```

## Themes

`spotify_player` uses `theme.toml` for custom themes.

Sample themes are available at [examples/theme.toml](../examples/theme.toml).

Select a theme by setting `theme` in `app.toml` or using the `-t <THEME>` / `--theme <THEME>` CLI flag.

A theme consists of:

- `name` (required): Theme name.
- `palette` (optional): Color palette.
- `component_style` (optional): Styles for UI components.

Omitted `palette` values use terminal colors. Omitted `component_style` values use default styles.

### Component Styles

The `component_style` table customizes UI component appearance. All fields are optional:

| Field                            | Description                                               |
| -------------------------------- | --------------------------------------------------------- |
| `block_title`                    | Style for block titles                                    |
| `border`                         | Style for borders                                         |
| `playback_status`                | Style for the playback status indicator                   |
| `playback_track`                 | Style for the currently playing track name                |
| `playback_artists`               | Style for the artist(s) of the current track              |
| `playback_album`                 | Style for the album name of the current track             |
| `playback_genres`                | Style for the genres of the current track                 |
| `playback_metadata`              | Style for the metadata section in playback                |
| `playback_progress_bar`          | Style for the filled portion of the playback progress bar |
| `playback_progress_bar_unfilled` | Style for the unfilled portion (only for `Line` type)     |
| `current_playing`                | Style for the currently playing item in lists             |
| `page_desc`                      | Style for the page description                            |
| `playlist_desc`                  | Style for the playlist description                        |
| `table_header`                   | Style for table headers                                   |
| `selection`                      | Style for selected items                                  |
| `secondary_row`                  | Style for secondary rows in tables/lists                  |
| `like`                           | Style for the like indicator                              |
| `lyrics_played`                  | Style for played lyrics lines                             |
| `lyrics_playing`                 | Style for the currently playing lyrics line               |

Each style accepts optional fields:

- `fg`: Foreground color
- `bg`: Background color
- `modifiers`: List of style modifiers

Defaults use palette values or remain unset if not specified.

#### Example

```toml
[[themes]]
name = "my_theme"
[themes.component_style]
block_title = { fg = "Magenta", modifiers = ["Bold"] }
border = { fg = "White" }
selection = { modifiers = ["Reversed", "Bold"] }
```

#### Default Component Styles

```toml
block_title = { fg = "Magenta"  }
border = {}
playback_status = { fg = "Cyan", modifiers = ["Bold"] }
playback_track = { fg = "Cyan", modifiers = ["Bold"] }
playback_artists = { fg = "Cyan", modifiers = ["Bold"] }
playback_album = { fg = "Yellow" }
playback_genres = { fg = "BrightBlack", modifiers = ["Italic"] }
playback_metadata = { fg = "BrightBlack" }
playback_progress_bar = { bg = "BrightBlack", fg = "Green" }
playback_progress_bar_unfilled = { bg = "BrightBlack" }
current_playing = { fg = "Green", modifiers = ["Bold"] }
page_desc = { fg = "Cyan", modifiers = ["Bold"] }
playlist_desc = { fg = "BrightBlack", modifiers = ["Dim"] }
table_header = { fg = "Blue" }
selection = { modifiers = ["Reversed", "Bold"] }
secondary_row = {}
like = {}
lyrics_played = { modifiers = ["Dim"] }
lyrics_playing = { fg = "Green", modifiers = ["Bold"] }
```

#### Accepted Colors

Colors can be:

- Black, Blue, Cyan, Green, Magenta, Red, White, Yellow
- BrightBlack, BrightWhite, BrightRed, BrightMagenta, BrightGreen, BrightCyan, BrightBlue, BrightYellow
- Hex codes: `#RRGGBB` (e.g., `#ff0000`)

#### Style Modifiers

Supported modifiers:

- Bold
- Dim
- Italic
- Underlined
- RapidBlink
- Reversed
- Hidden
- CrossedOut

Specify multiple modifiers as a list: `modifiers = ["Bold", "Underlined"]`.

### Use script to add theme

The [`theme_parse`](../scripts/theme_parse) Python script (requires `toml` and `requests`) converts [iTerm2/alacritty color schemes](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/alacritty) to compatible theme format.

Example:

```
./theme_parse "Builtin Solarized Dark" "solarized_dark"  >> ~/.config/spotify-player/theme.toml
```

This converts the [Builtin Solarized Dark](https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Builtin%20Solarized%20Dark.yml) color scheme to a theme named `solarized_dark`.

### Palette

A theme's `palette` table can include:

- `background`
- `foreground`
- `black`
- `blue`
- `cyan`
- `green`
- `magenta`
- `red`
- `white`
- `yellow`
- `bright_black`
- `bright_blue`
- `bright_cyan`
- `bright_green`
- `bright_magenta`
- `bright_red`
- `bright_white`
- `bright_yellow`

Omitted fields use terminal defaults. Values can be color names or hex codes. See [ANSI color reference](https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit).

## Keymaps

`spotify_player` uses `keymap.toml` to add or override [default key mappings](../README.md#commands). Add a `keymaps` entry to define a new mapping, or set the command to `None` to remove one. Example:

```toml
[[keymaps]]
command = "NextTrack"
key_sequence = "g n"
[[keymaps]]
command = "PreviousTrack"
key_sequence = "g p"
[[keymaps]]
command = "Search"
key_sequence = "C-c C-x /"
[[keymaps]]
command = "ResumePause"
key_sequence = "M-enter"
[[keymaps]]
command = "None"
key_sequence = "q"
[[keymaps]]
command = { VolumeChange = { offset = 1 } }
key_sequence = "-"
[[keymaps]]
command = { SeekForward = { duration = 10 } }
key_sequence = "E"
[[keymaps]]
command = { SeekBackward = { } }
key_sequence = "Q"
```

A complete list of actions is available [here](../README.md#actions).

## Actions

Actions are defined in `keymap.toml` and triggered by unbound key sequences. Actions target the selected item by default, but can be configured with `target` set to `PlayingTrack` or `SelectedItem`. See the [README](../README.md#actions) for available actions.

Example:

```toml
[[actions]]
action = "GoToArtist"
key_sequence = "g A"
[[actions]]
action = "GoToAlbum"
key_sequence = "g B"
target = "PlayingTrack"
[[actions]]
action="ToggleLiked"
key_sequence="C-l"
```
