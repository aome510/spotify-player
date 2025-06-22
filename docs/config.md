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

All configuration files should be placed inside the application's configuration folder (default to be `$HOME/.config/spotify-player`).

## General

**The default `app.toml` can be found in the example [`app.toml`](../examples/app.toml) file.**

`spotify_player` uses `app.toml` to configure general application configurations:

| Option                            | Description                                                                              | Default                                                 |
| --------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `client_id`                       | the Spotify client's ID                                                                  | `65b708073fc0480ea92a077233ca87bd`                      |
| `client_id_command`               | a shell command that prints the Spotify client ID to stdout (overrides `client_id`)      | `None`                                                  |
| `login_redirect_uri`              | the redirect URI for authenticating the application                                      | `http://127.0.0.1:8989/login`                           |
| `client_port`                     | the port that the application's client is running on to handle CLI commands              | `8080`                                                  |
| `tracks_playback_limit`           | the limit for the number of tracks played in a **tracks** playback                       | `50`                                                    |
| `playback_format`                 | the format of the text in the playback's window                                          | `{status} {track} • {artists}\n{album}\n{metadata}`     |
| `notify_format`                   | the format of a notification (`notify` feature only)                                     | `{ summary = "{track} • {artists}", body = "{album}" }` |
| `notify_timeout_in_secs`          | the timeout (in seconds) of a notification (`notify` feature only)                       | `0` (no timeout)                                        |
| `player_event_hook_command`       | the hook command executed when there is a new player event                               | `None`                                                  |
| `ap_port`                         | the application's Spotify session connection port                                        | `None`                                                  |
| `proxy`                           | the application's Spotify session connection proxy                                       | `None`                                                  |
| `theme`                           | the application's theme                                                                  | `default`                                               |
| `app_refresh_duration_in_ms`      | the duration (in ms) between two consecutive application refreshes                       | `32`                                                    |
| `playback_refresh_duration_in_ms` | the duration (in ms) between two consecutive playback refreshes                          | `0`                                                     |
| `page_size_in_rows`               | a page's size expressed as a number of rows (for page-navigation commands)               | `20`                                                    |
| `enable_media_control`            | enable application media control support (`media-control` feature only)                  | `true` (Linux), `false` (Windows and MacOS)             |
| `enable_streaming`                | enable streaming (`streaming` feature only)                                              | `Always`                                                |
| `enable_notify`                   | enable notification (`notify` feature only)                                              | `true`                                                  |
| `enable_cover_image_cache`        | store album's cover images in the cache folder                                           | `true`                                                  |
| `notify_streaming_only`           | only send notification when streaming is enabled (`streaming` and `notify` feature only) | `false`                                                 |
| `default_device`                  | the default device to connect to on startup if no playing device found                   | `spotify-player`                                        |
| `play_icon`                       | the icon to indicate playing state of a Spotify item                                     | `▶`                                                    |
| `pause_icon`                      | the icon to indicate pause state of a Spotify item                                       | `▌▌`                                                    |
| `liked_icon`                      | the icon to indicate the liked state of a song                                           | `♥`                                                    |
| `border_type`                     | the type of the application's borders                                                    | `Plain`                                                 |
| `progress_bar_type`               | the type of the playback progress bar                                                    | `Rectangle`                                             |
| `cover_img_width`                 | the width of the cover image (`image` feature only)                                      | `5`                                                     |
| `cover_img_length`                | the length of the cover image (`image` feature only)                                     | `9`                                                     |
| `cover_img_scale`                 | the scale of the cover image (`image` feature only)                                      | `1.0`                                                   |
| `seek_duration_secs`              | the duration (in seconds) to seek when using `SeekForward` and `SeekBackward` commands   | `5`                                                     |
| `sort_artist_albums_by_type`      | sort albums on artist's pages by type, i.e. album or single                              | `false`                                                 |

### Notes

- By default, `spotify_player` uses the official Spotify Web app's client (`client_id = 65b708073fc0480ea92a077233ca87bd`)
- It's recommended to specify [your own Client ID](https://developer.spotify.com/documentation/web-api/concepts/apps) to avoid possible rate limits and to allow a full [Spotify connect](https://www.spotify.com/us/connect/) support. An error such as `Failed to initialize the Spotify data` can appear if the `client_id` is invalid.
- `ap_port` and `proxy` are [Librespot's session configurations](https://github.com/librespot-org/librespot/wiki/Behind-web-proxy). By default, `spotify_player` doesn't set those values, which means the Librespot library will fallback to use its default options.
- Positive-value `app_refresh_duration_in_ms` is used to refresh the playback periodically. This can result in hitting a Spotify rate limit if the application is running for a long time.
- To prevent the rate limit, `spotify_player` sets `playback_refresh_duration_in_ms=0` by default and makes additional API calls when there is an event or a command triggering a playback update.
- List of commands that triggers a playback update:

  - `NextTrack`
  - `PreviousTrack`
  - `ResumePause`
  - `PlayRandom`
  - `Repeat`
  - `Shuffle`
  - `SeekTrack` (left-clicking the playback's progress bar)
  - `ChooseSelected` (for a track, a device, etc)

  **Note**: the above list might not be up-to-date.

- An example of event that triggers a playback update is the one happening when the current track ends.
- `enable_streaming` can be either `Always`, `Never` or `DaemonOnly`. For backwards compatibility, `true` and `false` are still accepted as aliases for `Always` and `Never`.
- `playback_window_position` can only be either `Top` or `Bottom`.
- `border_type` can be either `Hidden`, `Plain`, `Rounded`, `Double` or `Thick`.
- `progress_bar_type` can be either `Rectangle` or `Line`.
- `notify_streaming_only=true` and `enable_streaming=DaemonOnly` can be set to avoid sending multiple notifications when both daemon and UI are running.

#### Media control

Media control support (`enable_media_control` option) is enabled by default on Linux but disabled by default on MacOS and Windows.

MacOS and Windows require **an open window** to listen to OS media event. As a result, `spotify_player` needs to spawn an invisible window on startup, which may steal focus from the running terminal. To interact with `spotify_player`, which is run on the terminal, user will need to re-focus the terminal. Because of this extra re-focus step, the media control support is disabled by default on MacOS and Windows to avoid possible confusion for first-time users.

### Player event hook command

If specified, `player_event_hook_command` should be an object with two fields `command` and `args`. Each time `spotify_player` receives a new player event, `player_event_hook_command` is executed with the event's data as the script's arguments.

A player event is represented as a list of arguments with either of the following values:

- `"Changed" NEW_TRACK_ID`
- `"Playing" TRACK_ID POSITION_MS`
- `"Paused" TRACK_ID POSITION_MS`
- `"EndOfTrack" TRACK_ID`

**Note**: if `args` is specified, such arguments will be called before the event's arguments.

For example, if `player_event_hook_command = { command = "a.sh", args = ["-b", "c", "-d"] }`, upon receiving a `Changed` event with `NEW_TRACK_ID=id`, the following command will be run

```shell
a.sh -b c -d Changed id
```

Example script that reads event's data from arguments and prints them to a file:

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

If you prefer not to include your own `client_id` directly in your configuration, you can retrieve it at runtime using the `client_id_command` option.

If specified, `client_id_command` should be an object with two fields `command` and `args`, just like `player_event_hook_command`.
For example to read your client_id from a file your could use `client_id_command = { command = "cat", args = ["/path/to/file"] }`

> [!NOTE]
> When passing a path as an argument, always use the full path.
> The `~` symbol will not automatically expand to your home directory.

### Device configurations

The configuration options for the [Librespot](https://github.com/librespot-org/librespot) integrated device are specified under the `[device]` section in the `app.toml` file:

| Option          | Description                                                             | Default          |
| --------------- | ----------------------------------------------------------------------- | ---------------- |
| `name`          | The librespot device's name                                             | `spotify-player` |
| `device_type`   | The librespot device's type                                             | `speaker`        |
| `volume`        | Initial volume (in percentage) of the device                            | `70`             |
| `bitrate`       | Bitrate in kbps (`96`, `160`, or `320`)                                 | `320`            |
| `audio_cache`   | Enable caching audio files (store in `$APP_CACHE_FOLDER/audio/` folder) | `false`          |
| `normalization` | Enable audio normalization                                              | `false`          |
| `autoplay`      | Enable autoplay similar songs                                           | `false`          |

More details on the above configuration options can be found under the [Librespot wiki page](https://github.com/librespot-org/librespot/wiki/Options).

### Layout configurations

The layout of the application can be adjusted via these options.

| Option                     | Description                                          | Default |
| -------------------------- | ---------------------------------------------------- | ------- |
| `library.album_percent`    | The percentage of the album window in the library    | `40`    |
| `library.playlist_percent` | The percentage of the playlist window in the library | `40`    |
| `playback_window_position` | The position of the playback window                  | `Top`   |
| `playback_window_height`   | The height of the playback window                    | `6`     |

Example:

```toml

[layout]
library = { album_percent = 40, playlist_percent = 40 }
playback_window_position = "Top"

```

## Themes

`spotify_player` uses the `theme.toml` config file to look for user-defined themes.

**An example of user-defined themes can be found in the example [`theme.toml`](../examples/theme.toml) file.**

The application's theme can be modified by setting the `theme` config option in `app.toml` or by specifying the `-t <THEME>` (`--theme <THEME>`) CLI option when running the player.

A theme has three main components: `name` (the theme's name), `palette` (the theme's color palette), `component_style` (styles for specific application's components).

`name` is required when defining a new theme. If `palette` is not set, a palette based on the terminal's colors will be used. If `component_style` is not set, a set of predefined component styles will be used.

### Use script to add theme

[a `theme_parse` python script](../scripts/theme_parse) (require `toml` and `requests` libraries) can be used to parse [Iterm2 alacritty's color schemes](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/alacritty) into a `spotify_player` compatible theme format.

For example, you can run

```
./theme_parse "Builtin Solarized Dark" "solarized_dark"  >> ~/.config/spotify-player/theme.toml
```

to parse [Builtin Solarized Dark](https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Builtin%20Solarized%20Dark.yml) color scheme into a new theme with `name = "solarized_dark"`.

### Palette

A theme's palette consists of the following fields:

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

If a field is not specified, a default value based on the terminal's corresponding color will be used.

A field's value can be set to be either a hex representation of a RGB color (e.g, `background = "#1e1f29"`) or a string representation of the color (e.g `red`, `bright_blue`, etc).

More details about the palette's field naming can be found in the table in the [3-bit and 4-bit section](https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit).

### Component Styles

To define application's component styles, the user can specify any of the below fields:

- `block_title`
- `border`
- `playback_status`
- `playback_track`
- `playback_artists`
- `playback_album`
- `playback_metadata`
- `playback_progress_bar`
- `playback_progress_bar_unfilled` (Specific to `progress_bar_type` as `Line`)
- `current_playing`
- `page_desc`
- `table_header`
- `selection`
- `secondary_row`
- `like`
- `lyrics_played`
- `lyrics_playing`

A field in `component_style` is a struct with three **optional** fields: `fg` (foreground), `bg` (background) and `modifiers` (terminal effects):

- `fg` and `bg` can be either a palette's color in a pascal case (e.g, `BrightBlack`, `Blue`, etc) or a hex representation of a RGB color (e.g, `"#1e1f29"`). The default values for `fg` and `bg` are the `palette`'s `foreground` and `background`.
- The default value for `modifiers` is `[]`. `modifiers` can consist of
  - `Bold`
  - `Dim`
  - `Italic`
  - `Underlined`
  - `SlowBlink`
  - `Reversed`
  - `RapidBlink`
  - `Hidden`
  - `CrossedOut`

Default value for application's component styles:

```toml
block_title = { fg = "Magenta"  }
border = {}
playback_status = { fg = "Cyan", modifiers = ["Bold"] }
playback_track = { fg = "Cyan", modifiers = ["Bold"] }
playback_artists = { fg = "Cyan", modifiers = ["Bold"] }
playback_album = { fg = "Yellow" }
playback_metadata = { fg = "BrightBlack" }
playback_progress_bar = { bg = "BrightBlack", fg = "Green" }
current_playing = { fg = "Green", modifiers = ["Bold"] }
page_desc = { fg = "Cyan", modifiers = ["Bold"] }
playlist_desc = { fg = "BrightBlack", modifiers = ["Dim"] }
table_header = { fg = "Blue" }
secondary_row = {}
like = {}
lyrics_played = { modifiers = ["Dim"] }
lyrics_playing = { fg = "Green", modifiers = ["Bold"] }
```

## Keymaps

`spotify_player` uses `keymap.toml` to add or override new key mappings in additional to [the default key mappings](../README.md#commands). To define a new key mapping, simply add a `keymaps` entry. To remove a key mapping, set its command to `None`. For example,

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
```

## Actions

Actions are located in the same `keymap.toml` file as keymaps. An action can be triggered by a key sequence that is not bound to any command. Once the mapped key sequence is pressed, the corresponding action will be triggered. By default actions will act upon the currently selected item, you can change this behaviour by setting the `target` field for a keymap to either `PlayingTrack` or `SelectedItem`.
a list of actions can be found [here](../README.md#actions).

For example,

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
