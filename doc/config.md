# Configuration Documentation

## Table of Contents

- [General](#general)
- [Themes](#themes)
  - [Use script to add theme](#use-script-to-add-theme)
  - [Palette](#palette)
  - [Component Styles](#component-styles)
- [Keymaps](#keymaps)

All configurations are stored inside the application's configuration folder (default to be `$HOME/.config/spotify-player`).

## General

`spotify-player` uses `app.toml` to store general application configurations:

| Option                                     | Description                                                      | Default                            |
| ------------------------------------------ | ---------------------------------------------------------------- | ---------------------------------- |
| `client_id`                                | the application's client ID that interacts with Spotify APIs     | `65b708073fc0480ea92a077233ca87bd` |
| `theme`                                    | application's theme                                              | `dracula`                          |
| `n_refreshes_each_playback_update`         | number of refresh requests in each playback update               | `5`                                |
| `refresh_delay_in_ms_each_playback_update` | delay in ms between two refresh requests in each playback update | `500`                              |
| `app_refresh_duration_in_ms`               | duration in ms for re-rendering the application's UI             | `100`                              |
| `playback_refresh_duration_in_ms`          | duration in ms for refreshing the player's playback periodically | `0`                                |
| `track_table_item_max_len`                 | maximum length for a column in a track table                     | `32`                               |

**Note**:

- By default, the application uses the official Spotify Web app's client ID (`65b708073fc0480ea92a077233ca87bd`). It's recommended to use [your own Client ID](https://developer.spotify.com/documentation/general/guides/app-settings/) to avoid possible rate limit and to allow a full [Spotify connect](https://www.spotify.com/us/connect/) support.
- Positive-value `app_refresh_duration_in_ms` is used to refresh the current playback (making a Spotify API call) every `app_refresh_duration_in_ms` ms. This can result in hitting Spotify rate limit if the player is running for a long period of time.
- To prevent the rate limit, `spotify-player` sets `playback_refresh_duration_in_ms=0` by default and relies on `n_refreshes_each_playback_update` and `refresh_delay_in_ms_each_playback_update` for refreshing the playback each time a command or event updates the player's playback.
- List of commands that triggers a playback update:
  - `NextTrack`
  - `PreviousTrack`
  - `ResumePause`
  - `PlayRandom`
  - `Repeat`
  - `Shuffle`
  - `SeekTrack` (left-clicking the playback's progress bar)
  - `ChooseSelected` (for a track, a device, etc)
- The playback is also updated when the current track ends (using a timer based on the track's duration).

### Device configurations

[Librespot](https://github.com/librespot-org/librespot) device configuration options are configured under the `[device]` section in the `app.toml` file:

| Option        | Description                                                      | Default          |
| ------------- | ---------------------------------------------------------------- | ---------------- |
| `name`        | The librespot device's name                                      | `spotify-player` |
| `device_type` | The librespot device's type displayed in Spotify clients         | `speaker`        |
| `volume`      | Initial volume (in percentage) of the device                     | `50`             |
| `bitrate`     | Bitrate in kbps (`96`, `160`, `320`)                             | `160`            |
| `audio_cache` | Enable caching audio files (store in `$APP_CACHE_FOLDER/audio/`) | `false`          |

More details on the above configuration options can be found under the [Librespot wiki page](https://github.com/librespot-org/librespot/wiki/Options).

## Themes

`spotify-player` uses `theme.toml` to define additional themes in addition to the default themes (`dracula`, `ayu_light`, `gruvbox_dark`, `solarized_light`).

The new theme can then be used by setting the `theme` option in the [`app.toml`](#general) file or specifying the `-t <THEME>` (`--theme <THEME>`) option when running the player.

A theme has three main components: `name` (the theme's name), `palette` (the theme's color palette), `component_style` (a list of predefined style for application's components). `name` and `palette` are required when defining a new theme. If `component_style` is not specified, a default value will be used.

An example of user-defined themes can be found in the example [`theme.toml`](https://github.com/aome510/spotify-player/blob/master/examples/theme.toml) file

### Use script to add theme

I have created [a `theme_parse` python script](../scripts/theme_parse) (require `pyaml` and `requests` libraries) to parse [Iterm2 alacritty's color schemes](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/alacritty) into `spotify-player` compatible theme configurations.

For example, you can run

```
./theme_parse "Builtin Solarized Dark" "solarized_dark"  >> ~/.config/spotify-player/theme.toml
```

to parse [Builtin Solarized Dark](https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Builtin%20Solarized%20Dark.yml) color scheme into a new theme with `name = "solarized_dark"`.

### Palette

To define a theme's color palette, user needs to specify **all** the below fields:

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
- `selection_background`
- `selection_foreground`

A field in the color palette must be set to the hex representation of a RGB color. For example, `background = "#1e1f29"`.

### Component Styles

To define application's component styles, user needs to specify **all** the below fields:

- `block_title`
- `playback_track`
- `playback_album`
- `playback_metadata`
- `playback_progress_bar`
- `current_active`
- `context_desc`
- `context_tracks_table_header`

A field in the component styles is a `Style` struct which has three optional fields: `fg`, `bg` and `modifiers`. `fg` and `bg` can be either a palette's color (string in pascal case) or a custom RGB color using the following format: `fg = { Rgb { r = 0, g = 0, b = 0} }`. `modifiers` can only be either `Italic` or `Bold`.

Default value for application's component styles:

```toml
block_title = { fg = "Magenta"  }
playback_track = { fg = "Cyan", modifiers = ["Bold"] }
playback_album = { fg = "Yellow" }
playback_metadata = { fg = "BrightBlack" }
playback_progress_bar = { fg = "SelectionBackground", bg = "Green", modifiers = ["Italic"] }
current_active = { fg = "Green", modifiers = ["Bold"] }
context_desc = { fg = "Cyan", modifiers = ["Bold"] }
context_tracks_table_header = { fg = "Blue" }
```

## Keymaps

`spotify-player` uses `keymap.toml` to add or override new key mappings in additional to [the default key mappings](https://github.com/aome510/spotify-player#commands). To define a new key mapping, simply add a `keymaps` entry. For example,

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
```
