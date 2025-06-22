# spotify_player

## Table of Contents

- [Introduction](#introduction)
- [Examples](#examples)
- [Installation](#installation)
- [Features](#features)
  - [Spotify Connect](#spotify-connect)
  - [Streaming](#streaming)
  - [Media Control](#media-control)
  - [Image](#image)
  - [Notify](#notify)
  - [Mouse support](#mouse-support)
  - [Daemon](#daemon)
  - [Fuzzy search](#fuzzy-search)
  - [CLI commands](#cli-commands)
- [Commands](#commands)
- [Configurations](#configurations)
- [Caches](#caches)
  - [Logging](#logging)
- [Acknowledgement](#acknowledgement)

## Introduction

`spotify_player` is a fast, easy to use, and configurable terminal music player.

**Features**

- Minimalist UI with an intuitive paging and popup system.
- Highly [configurable](docs/config.md)
- Feature parity with the official Spotify application.
- Support remote control with [Spotify Connect](#spotify-connect).
- Support [streaming](#streaming) songs directly from the terminal.
- Support synced lyrics.
- Support [cross-platform media control](#media-control).
- Support [image rendering](#image).
- Support [desktop notification](#notify).
- Support running the application as [a daemon](#daemon)
- Offer a wide range of [CLI commands](#cli-commands)

## Examples

A demo of `spotify_player` `v0.5.0-pre-release` on [youtube](https://www.youtube.com/watch/Jbfe9GLNWbA) or on [asciicast](https://asciinema.org/a/446913):

Checkout [examples/README.md](./examples/README.md) for more examples.

## Installation

By default, the application's installed binary is `spotify_player`.

### Requirements

A Spotify Premium account is **required**.

#### Dependencies

##### Windows and MacOS

- [Rust and cargo](https://www.rust-lang.org/tools/install) as the build dependencies

##### Linux

- [Rust and cargo](https://www.rust-lang.org/tools/install) as the build dependencies
- install `openssl`, `alsa-lib` (`streaming` feature), `libdbus` (`media-control` feature).

  - For example, on Debian based systems, run the below command to install application's dependencies:

    ```shell
    sudo apt install libssl-dev libasound2-dev libdbus-1-dev
    ```
  - On RHEL/Fedora based systems, run the below command to install application's dependencies :

    ```shell
    sudo dnf install openssl-devel alsa-lib-devel dbus-devel
    ```
    or if you're using `yum`:
    ```shell
    sudo yum install openssl-devel alsa-lib-devel dbus-devel
    ```

### Binaries

Application's prebuilt binaries can be found in the [Releases Page](https://github.com/aome510/spotify-player/releases).

**Note**: to run the application, Linux systems need to install additional dependencies as specified in the [Dependencies section](#linux).

### Homebrew

Run `brew install spotify_player` to install the application.

### Scoop

Run `scoop install spotify-player` to install the application.

### Cargo

Run `cargo install spotify_player --locked` to install the application from [crates.io](https://crates.io/crates/spotify_player).

### Arch Linux

Run `pacman -S spotify-player` to install the application.

Alternatively, run `yay -S spotify-player-full` to install an AUR package compiled with full feature support and Pulseaudio/Pipewire instead of rodio.

### Void Linux

Run `xbps-install -S spotify-player` to install the application.

### FreeBSD

Run `pkg install spotify-player` to install the `spotify_player` binary from FreeBSD ports.

### NetBSD

Using the package manager, run `pkgin install spotify-player` to install from the official repositories.

Building from source,

```
cd /usr/pkgsrc/audio/spotify-player
make install
```

### NixOS

[spotify-player](https://search.nixos.org/packages?channel=unstable&show=spotify-player&from=0&size=50&sort=relevance&type=packages&query=spotify-player) is available as a Nix package and can be installed via `nix-shell -p spotify-player` or as part of your system configuration.

If you want to build the source locally you can run `nix-shell` in the root of a checkout of the source code. The provided `shell.nix` file will install the build prerequisites.

### Docker

**Note**: [streaming](#streaming) feature is disabled when using the docker image.

You can download the binary image of the latest build from the `master` branch by running

```
docker pull aome510/spotify_player:latest
```

then run

```
docker run --rm -it aome510/spotify_player:latest
```

to run the application.

You can also use your local config folder to configure the application or your local cache folder to store the application's cache data when running the docker image:

```
docker run --rm \
-v $APP_CONFIG_FOLDER:/app/config/ \
-v $APP_CACHE_FOLDER:/app/cache/ \
-it aome510/spotify_player:latest
```

## Features

### Spotify Connect

To enable a full [Spotify connect](https://www.spotify.com/us/connect/) support, user will need to register a Spotify application and specify the application's `client_id` in the general configuration file as described in the [configuration documentation](docs/config.md#general).

More details about registering a Spotify application can be found in the [official Spotify documentation](https://developer.spotify.com/documentation/general/guides/authorization/app-settings/).

When `spotify_player` runs with your own `client_id`, press **D** (default shortcut for `SwitchDevice` command) to get the list of available devices, then press **enter** (default shortcut for `ChooseSelected` command) to connect to the selected device.

### Streaming

`spotify_player` supports streaming, which needs to be built/installed with `streaming` feature (**enabled** by default) **and** with an audio backend (`rodio-backend` by default). The streaming feature allows to `spotify_player` to play music directly from terminal.

The application uses [librespot](https://github.com/librespot-org/librespot) library to create an integrated Spotify client while running. The integrated client will register a Spotify speaker device under the `spotify-player` name, which is accessible on the [Spotify connect](#spotify-connect) device list.

#### Audio backend

`spotify_player` uses [rodio](https://github.com/RustAudio/rodio) as the default [audio backend](https://github.com/librespot-org/librespot/wiki/Audio-Backends). List of available audio backends:

- `alsa-backend`
- `pulseaudio-backend`
- `rodio-backend`
- `portaudio-backend`
- `jackaudio-backend`
- `rodiojack-backend`
- `sdl-backend`
- `gstreamer-backend`

User can change the audio backend when building/installing the application by specifying the `--features` option. For example, to install `spotify_player` with `pulseaudio-backend`, run

```shell
cargo install spotify_player --no-default-features --features pulseaudio-backend
```

**Note**:

- needs to specify `--no-default-features` here because `rodio-backend` is one of the default features.
- user will need to install additional dependencies depending on the selected audio backend. More details can be found in the [Librespot documentation](https://github.com/librespot-org/librespot/wiki/Compiling#general-dependencies).

The `streaming` feature can be also disabled upon installing by running

```shell
cargo install spotify_player --no-default-features
```

### Media Control

To enable media control support, `spotify_player` needs to be built/installed with `media-control` feature (**enabled** by default) and set the `enable_media_control` config option to `true` in the [general configuration file](docs/config.md#media-control).

Media control support is implemented using [MPRIS DBus](https://wiki.archlinux.org/title/MPRIS) on Linux and OS window event listener on Windows and MacOS.

### Image

To enable image rendering support, `spotify_player` needs to be built/installed with `image` feature (**disabled** by default). To install the application with `image` feature included, run:

```shell
cargo install spotify_player --features image
```

`spotify_player` supports rendering image in a full resolution if the application is run on either [Kitty](https://sw.kovidgoyal.net/kitty/graphics-protocol/) or [iTerm2](https://iterm2.com/documentation-images.html). Otherwise, the image will be displayed as [block characters](https://en.wikipedia.org/wiki/Block_Elements).

`spotify_player` also supports rendering images with `sixel` behind `sixel` feature flag, which also enables `image` feature:

```shell
cargo install spotify_player --features sixel
```

**Notes**:

- Not all terminals supported by [libsixel](https://github.com/saitoha/libsixel) are supported by `spotify_player` as it relies on a [third-party library](https://github.com/atanunq/viuer) for image rendering. A possible list of supported terminals can be found in [here](https://github.com/atanunq/viuer/blob/dc81f44a97727e04be0b000712e9233c92116ff8/src/printer/sixel.rs#L83-L95).
- Images rendered by `sixel` can have a _weird_ scale. It's recommended to tweak the `cover_img_scale` config option to get the best result as the scaling works differently with different terminals and fonts.

Examples of image rendering:

- iTerm2:

![iTerm2](https://user-images.githubusercontent.com/40011582/172966798-0aadc431-b0c3-4433-adf3-7526684fc2a0.png)

- Kitty:

![kitty](https://user-images.githubusercontent.com/40011582/172967028-8cfb2daa-1642-499a-a5bf-8ed77f2b3fac.png)

- Sixel (`foot` terminal, `cover_img_scale=1.8`):

![sixel](https://user-images.githubusercontent.com/40011582/219880331-58ac1c30-bbb0-4c99-a6cc-e5b7c9c81455.png)

- Others:

![others](https://user-images.githubusercontent.com/40011582/172967325-d2098037-e19e-440a-a38a-5b076253ecb1.png)

### Notify

To enable desktop notification support, `spotify_player` needs to be built/installed with `notify` feature (**disabled** by default). To install the application with `notify` feature included, run:

```shell
cargo install spotify_player --features notify
```

**Note**: the notification support in `MacOS` and `Windows` are quite restricted compared to `Linux`.

### Mouse support

Currently, the only supported use case for mouse is to seek to a position of the current playback by left-clicking to such position in the playback's progress bar.

### Daemon

To enable a [daemon](<https://en.wikipedia.org/wiki/Daemon_(computing)>) support, `spotify_player` needs to be built/installed with `daemon` feature (**disabled** by default). To install the application with `daemon` feature included, run:

```shell
cargo install spotify_player --features daemon
```

You can run the application as a daemon by specifying the `-d` or `--daemon` option: `spotify_player -d`.

**Notes**:

- `daemon` feature is not supported on Windows
- `daemon` feature requires the `streaming` feature to be enabled and the application to be built with [an audio backend](#audio-backend)
- because of the OS's restrictions, `daemon` feature doesn't work with the `media-control` feature on MacOS, which is **enabled by default**. In other words, if you want to use the `daemon` feature on MacOS, you must install the application with `media-control` feature **disabled**:

  ```shell
  cargo install spotify_player --no-default-features --features daemon,rodio-backend
  ```

### Fuzzy search

To enable [fuzzy search](https://en.wikipedia.org/wiki/Approximate_string_matching) support, `spotify_player` needs to be built/installed with `fzf` feature (**disabled** by default).

### CLI Commands

`spotify_player` offers several CLI commands to interact with Spotify:

- `get`: Get Spotify data (playlist/album/artist data, user's data, etc)
- `playback`: Interact with the playback (start a playback, play-pause, next, etc)
- `search`: Search spotify
- `connect`: Connect to a Spotify device
- `like`: Like currently playing track
- `authenticate`: Authenticate the application
- `playlist`: Playlist editing (new, delete, import, fork, etc)

For more details, run `spotify_player -h` or `spotify_player {command} -h`, in which `{command}` is a CLI command.

**Notes**

- When using the CLI for the first time, you'll need to run `spotify_player authenticate` to authenticate the application beforehand.
- Under the hood, CLI command is handled by sending requests to a `spotify_player` client socket running on port `client_port`, [a general application configuration](https://github.com/aome510/spotify-player/blob/master/docs/config.md#general) with a default value of `8080`. If there is no running application's instance, a new client will be created upon handling the CLI commands, which increases the latency of the command.

#### Scripting

The `spotify_player` command-line interface makes scripting easy.
With the `search` subcommand, you can search Spotify and retrieve data in JSON format, enabling queries with tools like [jq](https://jqlang.github.io/jq/).

Here’s an example of starting playback for the first track from a search query:

```sh
read -p "Search spotify: " query
spotify_player playback start track --id $(spotify_player search "$query" | jq '.tracks.[0].id' | xargs)
```

## Commands

To go to the shortcut help page, press `?` or `C-h` (default shortcuts for `OpenCommandHelp` command).

**Tips**:

- you can search in the shortcut help page (and some other pages) using `Search` command
- `RefreshPlayback` can be used to manually update the playback status.
- `RestartIntegratedClient` is useful when user wants to switch to another audio device (headphone, earphone, etc) without restarting the application, as the integrated client will be re-initialized with the new device.

List of supported commands:

| Command                        | Description                                                                                        | Default shortcuts  |
| ------------------------------ | -------------------------------------------------------------------------------------------------- | ------------------ |
| `NextTrack`                    | next track                                                                                         | `n`                |
| `PreviousTrack`                | previous track                                                                                     | `p`                |
| `ResumePause`                  | resume/pause based on the current playback                                                         | `space`            |
| `PlayRandom`                   | play a random track in the current context                                                         | `.`                |
| `Repeat`                       | cycle the repeat mode                                                                              | `C-r`              |
| `ToggleFakeTrackRepeatMode`    | toggle fake track repeat mode                                                                      | `M-r`              |
| `Shuffle`                      | toggle the shuffle mode                                                                            | `C-s`              |
| `VolumeChange`                 | change playback volume by an offset (default shortcuts use 5%)                                     | `+`, `-`           |
| `Mute`                         | toggle playback volume between 0% and previous level                                               | `_`                |
| `SeekForward`                  | seek forward by 5s                                                                                 | `>`                |
| `SeekBackward`                 | seek backward by 5s                                                                                | `<`                |
| `Quit`                         | quit the application                                                                               | `C-c`, `q`         |
| `ClosePopup`                   | close a popup                                                                                      | `esc`              |
| `SelectNextOrScrollDown`       | select the next item in a list/table or scroll down (supports vim-style count: 5j)                 | `j`, `C-n`, `down` |
| `SelectPreviousOrScrollUp`     | select the previous item in a list/table or scroll up (supports vim-style count: 10k)              | `k`, `C-p`, `up`   |
| `PageSelectNextOrScrollDown`   | select the next page item in a list/table or scroll a page down (supports vim-style count: 3C-f)   | `page_down`, `C-f` |
| `PageSelectPreviousOrScrollUp` | select the previous page item in a list/table or scroll a page up (supports vim-style count: 2C-b) | `page_up`, `C-b`   |
| `SelectFirstOrScrollToTop`     | select the first item in a list/table or scroll to the top                                         | `g g`, `home`      |
| `SelectLastOrScrollToBottom`   | select the last item in a list/table or scroll to the bottom                                       | `G`, `end`         |
| `ChooseSelected`               | choose the selected item                                                                           | `enter`            |
| `RefreshPlayback`              | manually refresh the current playback                                                              | `r`                |
| `RestartIntegratedClient`      | restart the integrated client (`streaming` feature only)                                           | `R`                |
| `ShowActionsOnSelectedItem`    | open a popup showing actions on a selected item                                                    | `g a`, `C-space`   |
| `ShowActionsOnCurrentTrack`    | open a popup showing actions on the current track                                                  | `a`                |
| `AddSelectedItemToQueue`       | add the selected item to queue                                                                     | `Z`, `C-z`         |
| `FocusNextWindow`              | focus the next focusable window (if any)                                                           | `tab`              |
| `FocusPreviousWindow`          | focus the previous focusable window (if any)                                                       | `backtab`          |
| `SwitchTheme`                  | open a popup for switching theme                                                                   | `T`                |
| `SwitchDevice`                 | open a popup for switching device                                                                  | `D`                |
| `Search`                       | open a popup for searching in the current page                                                     | `/`                |
| `BrowseUserPlaylists`          | open a popup for browsing user's playlists                                                         | `u p`              |
| `BrowseUserFollowedArtists`    | open a popup for browsing user's followed artists                                                  | `u a`              |
| `BrowseUserSavedAlbums`        | open a popup for browsing user's saved albums                                                      | `u A`              |
| `CurrentlyPlayingContextPage`  | go to the currently playing context page                                                           | `g space`          |
| `TopTrackPage`                 | go to the user top track page                                                                      | `g t`              |
| `RecentlyPlayedTrackPage`      | go to the user recently played track page                                                          | `g r`              |
| `LikedTrackPage`               | go to the user liked track page                                                                    | `g y`              |
| `LyricsPage`                   | go to the lyrics page of the current track                                                         | `g L`, `l`         |
| `LibraryPage`                  | go to the user library page                                                                        | `g l`              |
| `SearchPage`                   | go to the search page                                                                              | `g s`              |
| `BrowsePage`                   | go to the browse page                                                                              | `g b`              |
| `Queue`                        | go to the queue page                                                                               | `z`                |
| `OpenCommandHelp`              | go to the command help page                                                                        | `?`, `C-h`         |
| `PreviousPage`                 | go to the previous page                                                                            | `backspace`, `C-q` |
| `OpenSpotifyLinkFromClipboard` | open a Spotify link from clipboard                                                                 | `O`                |
| `SortTrackByTitle`             | sort the track table (if any) by track's title                                                     | `s t`              |
| `SortTrackByArtists`           | sort the track table (if any) by track's artists                                                   | `s a`              |
| `SortTrackByAlbum`             | sort the track table (if any) by track's album                                                     | `s A`              |
| `SortTrackByAddedDate`         | sort the track table (if any) by track's added date                                                | `s D`              |
| `SortTrackByDuration`          | sort the track table (if any) by track's duration                                                  | `s d`              |
| `SortLibraryAlphabetically`    | sort the library alphabetically                                                                    | `s l a`            |
| `SortLibraryByRecent`          | sort the library (playlists and albums) by recently added items                                    | `s l r`            |
| `ReverseOrder`                 | reverse the order of the track table (if any)                                                      | `s r`              |
| `MovePlaylistItemUp`           | move playlist item up one position                                                                 | `C-k`              |
| `MovePlaylistItemDown`         | move playlist item down one position                                                               | `C-j`              |
| `CreatePlaylist`               | create a new playlist                                                                              | `N`                |
| `JumpToCurrentTrackInContext`  | jump to the current track in the context                                                           | `g c`              |

To add new shortcuts or modify the default shortcuts, please refer to the [keymaps section](docs/config.md#keymaps) in the configuration documentation.

### Actions

A general list of actions is available; however, not all Spotify items (track, album, artist, or playlist) implement each action. To get the list of available actions on an item, call the `ShowActionsOnCurrentTrack` command or the `ShowActionsOnSelectedItem` command, then press enter (default binding for the `ChooseSelected` command) to initiate the selected action. Some actions may not appear in the popup but can be bound to a shortcut.

List of available actions:

- `GoToArtist`
- `GoToAlbum`
- `GoToRadio`
- `AddToLibrary`
- `AddToPlaylist`
- `AddToQueue`
- `AddToLiked`
- `DeleteFromLiked`
- `DeleteFromLibrary`
- `DeleteFromPlaylist`
- `ShowActionsOnAlbum`
- `ShowActionsOnArtist`
- `ShowActionsOnShow`
- `ToggleLiked`
- `CopyLink`
- `Follow`
- `Unfollow`

These actions can also be bound to a shortcut. To add new shortcuts, please refer to the [actions section](docs/config.md#actions) in the configuration documentation.

### Search Page

When first entering the search page, the application focuses on the search input. User can then input text, delete one character backward using `backspace`, or search the text using `enter`.

To move the focus from the search input to the other windows such as track results, album results, etc, use `FocusNextWindow` or `FocusPreviousWindow`.

## Configurations

By default, `spotify_player` will look into `$HOME/.config/spotify-player` for application's configuration files. This can be changed by either specifying `-c <FOLDER_PATH>` or `--config-folder <FOLDER_PATH>` option.

If an application configuration file is not found, one will be created with default values.

Please refer to [the configuration documentation](docs/config.md) for more details on the configuration options.

## Caches

By default, `spotify_player` will look into `$HOME/.cache/spotify-player` for application's cache files, which include log files, Spotify's authorization credentials, audio cache files, etc. This can be changed by either specifying `-C <FOLDER_PATH>` or `--cache-folder <FOLDER_PATH>` option.

### Logging

The application stores logs inside the `$APP_CACHE_FOLDER/spotify-player-*.log` file. For debugging or submitting an issue, user can also refer to the backtrace file in `$APP_CACHE_FOLDER/spotify-player-*.backtrace`, which includes the application's backtrace in case of panics/unexpected errors.

`spotify_player` uses `RUST_LOG` environment variable to define the application's [logging level](https://docs.rs/log/0.4.14/log/enum.Level.html). `RUST_LOG` is default to be `spotify_player=INFO`, which only shows the application's logs.

## Acknowledgement

`spotify_player` is written in [Rust](https://www.rust-lang.org) and is built on top of awesome libraries such as [ratatui](https://github.com/ratatui/ratatui), [rspotify](https://github.com/ramsayleung/rspotify), [librespot](https://github.com/librespot-org/librespot), and [many more](spotify_player/Cargo.toml). It's highly inspired by [spotify-tui](https://github.com/Rigellute/spotify-tui) and [ncspot](https://github.com/hrkfdn/ncspot).
