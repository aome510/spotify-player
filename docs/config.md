# Configuration Documentation

## Table of Contents

- [General](#general)
  - [Notes](#notes)
  - [Device configurations](#device-configurations)
- [Themes](#themes)
  - [Use script to add theme](#use-script-to-add-theme)
  - [Palette](#palette)
  - [Component Styles](#component-styles)
- [Keymaps](#keymaps)

All configuration files should be placed inside the application's configuration folder (default to be `$HOME/.config/spotify-player`).

## General

**The default `app.toml` can be found in the example [`app.toml`](../examples/app.toml) file.**

`spotify_player` uses `app.toml` to configure general application configurations:

| Option                               | Description                                                                   | Default                                                    |
| ------------------------------------ | ----------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `client_id`                          | the Spotify client's ID                                                       | `65b708073fc0480ea92a077233ca87bd`                         |
| `client_port`                        | the port that the application's client is running on to handle CLI commands   | `8080`                                                     |
| `tracks_playback_limit`              | the limit for the number of tracks played in a **tracks** playback            | `50`                                                       |
| `playback_format`                    | the format of the text in the playback's window                               | `{track} • {artists}\n{album}\n{metadata}`                 |
| `notify_format`                      | the format of a notification (`notify` feature only)                          | `{ summary = "{track} • {artists}", body = "{album}" }`    |
| `copy_command`                       | the command used to execute a copy-to-clipboard action                        | `xclip -sel c` (Linux), `pbcopy` (MacOS), `clip` (Windows) |
| `ap_port`                            | the application's Spotify session connection port                             | `None`                                                     |
| `proxy`                              | the application's Spotify session connection proxy                            | `None`                                                     |
| `theme`                              | the application's theme                                                       | `default`                                                  |
| `app_refresh_duration_in_ms`         | the duration (in ms) between two consecutive application refreshes            | `32`                                                       |
| `playback_refresh_duration_in_ms`    | the duration (in ms) between two consecutive playback refreshes               | `0`                                                        |
| `cover_image_refresh_duration_in_ms` | the duration (in ms) between two cover image refreshes (`image` feature only) | `2000`                                                     |
| `page_size_in_rows`                  | a page's size expressed as a number of rows (for page-navigation commands)    | `20`                                                       |
| `track_table_item_max_len`           | the maximum length of a column in a track table                               | `32`                                                       |
| `enable_media_control`               | enable application media control support (`media-control` feature only)       | `true` (Linux), `false` (Windows and MacOS)                |
| `enable_streaming`                   | create a device for streaming (streaming feature only)                        | `true`                                                     |
| `enable_cover_image_cache`           | store album's cover images in the cache folder                                | `true`                                                     |
| `default_device`                     | the default device to connect to on startup if no playing device found        | `spotify-player`                                           |
| `play_icon`                          | the icon to indicate playing state of a Spotify item                          | `▶`                                                        |
| `pause_icon`                         | the icon to indicate pause state of a Spotify item                            | `▌▌`                                                       |
| `liked_icon`                         | the icon to indicate the liked state of a song                                | `♥`                                                        |
| `border_type`                        | the type of the application's borders                                         | `Plain`                                                    |
| `progress_bar_type`                  | the type of the playback progress bar                                         | `Rectangle`                                                |
| `playback_window_position`           | the position of the playback window                                           | `Top`                                                      |
| `playback_window_width`              | the width of the playback window                                              | `6`                                                        |
| `cover_img_width`                    | the width of the cover image (`image` feature only)                           | `5`                                                        |
| `cover_img_length`                   | the length of the cover image (`image` feature only)                          | `9`                                                        |
| `cover_img_scale`                    | the scale of the cover image (`image` feature only)                           | `1.0`                                                      |

### Notes

- By default, `spotify_player` uses the official Spotify Web app's client (`client_id = 65b708073fc0480ea92a077233ca87bd`)
- It's recommended to specify [your own Client ID](https://developer.spotify.com/documentation/web-api/concepts/apps) to avoid possible rate limits and to allow a full [Spotify connect](https://www.spotify.com/us/connect/) support.
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
- `copy_command` is represented by a struct with two fields `command` and `args`. For example, `copy_command = { command = "xclip", args = ["-sel", "c"] }`. The copy command should read input from **standard input**.
- `playback_window_position` can only be either `Top` or `Bottom`.
- `border_type` can be either `Hidden`, `Plain`, `Rounded`, `Double`, `Thick`.
- `progress_bar_type` can be either `Rectangle` or `Line`.

#### Media control

Media control support (`enable_media_control` option) is enabled by default on Linux but disabled by default on MacOS and Windows.

MacOS and Windows require **an open window** to listen to OS media event. As a result, `spotify_player` needs to spawn an invisible window on startup, which may steal focus from the running terminal. To interact with `spotify_player`, which is run on the terminal, user will need to re-focus the terminal. Because of this extra re-focus step, the media control support is disabled by default on MacOS and Windows to avoid possible confusion for first-time users.

### Device configurations

The configuration options for the [Librespot](https://github.com/librespot-org/librespot) integrated device are specified under the `[device]` section in the `app.toml` file:

| Option        | Description                                                             | Default          |
| ------------- | ----------------------------------------------------------------------- | ---------------- |
| `name`        | The librespot device's name                                             | `spotify-player` |
| `device_type` | The librespot device's type                                             | `speaker`        |
| `volume`      | Initial volume (in percentage) of the device                            | `50`             |
| `bitrate`     | Bitrate in kbps (`96`, `160`, or `320`)                                 | `160`            |
| `audio_cache` | Enable caching audio files (store in `$APP_CACHE_FOLDER/audio/` folder) | `false`          |

More details on the above configuration options can be found under the [Librespot wiki page](https://github.com/librespot-org/librespot/wiki/Options).

## Themes

`spotify_player` uses `theme.toml` to look for user-defined themes.

**An example of user-defined themes can be found in the example [`theme.toml`](../examples/theme.toml) file.**

The application's theme can be modified by setting the `theme` option in `app.toml` or by specifying the `-t <THEME>` (`--theme <THEME>`) option when running the player.

A theme has three main components: `name` (the theme's name), `palette` (the theme's color palette), `component_style` (a list of pre-defined styles for application's components).

`name` is required when defining a new theme. If `palette` is not set, a palette based on the terminal's colorscheme will be used. If `component_style` is not specified, a default value will be used.

### Use script to add theme

[a `theme_parse` python script](../scripts/theme_parse) (require `pyaml` and `requests` libraries) can be used to parse [Iterm2 alacritty's color schemes](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/alacritty) into `spotify_player` compatible theme configurations.

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

If a field is not specified, its default value will be based on the terminal's corresponding color.
If specified, a field's value must be set to be a hex representation of a RGB color. For example, `background = "#1e1f29"`.

### Component Styles

To define application's component styles, the user can specify any of the below fields:

- `block_title`
- `border`
- `playback_track`
- `playback_artists`
- `playback_album`
- `playback_metadata`
- `playback_progress_bar`
- `current_playing`
- `page_desc`
- `table_header`
- `selection`

A field in the component styles is a `Style` struct which has three optional fields: `fg`, `bg` and `modifiers`. `fg` and `bg` can be either a palette's color (string in pascal case) or a custom RGB color using the following format: `fg = { Rgb { r = ..., g = ..., b = ... } }`. The default values for `fg` and `bg` are the `palette`'s `fg` and `bg`. `modifiers` can only be `Italic`, `Bold` or `Reversed`.

Default value for application's component styles:

```toml
block_title = { fg = "Magenta"  }
border = {}
playback_track = { fg = "Cyan", modifiers = ["Bold"] }
playback_artists = { fg = "Cyan", modifiers = ["Bold"] }
playback_album = { fg = "Yellow" }
playback_metadata = { fg = "BrightBlack" }
playback_progress_bar = { bg = "BrightBlack", fg = "Green" }
current_playing = { fg = "Green", modifiers = ["Bold"] }
page_desc = { fg = "Cyan", modifiers = ["Bold"] }
table_header = { fg = "Blue" }
selection = { modifiers = ["Bold", "Reversed"] }
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
command = "SearchContext"
key_sequence = "C-c C-x /"
[[keymaps]]
command = "ResumePause"
key_sequence = "M-enter"
[[keymaps]]
command = "None"
key_sequence = "q"
```
