# spotify-player

## Table of Contents

- [Introduction](#introduction)
- [Examples](#examples)
  - [Demo](#demo)
- [Installation](#installation)
  - [Requirements](#requirements)
- [Spotify Connect](#spotify-connect)
- [Streaming](#streaming)
- [Commands](#commands)
  - [Actions](#actions)
  - [Search Page](#search-page)
- [Mouse support](#mouse-support)
- [Configurations](#configurations)
- [Caches](#caches)
  - [Logging](#logging)
- [Roadmap](#roadmap)

## Introduction

- `spotify-player` is a fast, easy to use, and [configurable](https://github.com/aome510/spotify-player/blob/master/doc/config.md) terminal music player.
- `spotify-player` is designed to be a player, not a fully-fledged Spotify clone, so it does not aim to support all Spotify features. Its main goal is to provide a quick and intuitive way to control music using [commands](#commands).
- `spotify-player` is built on top of awesome libraries such as [tui-rs](https://github.com/fdehau/tui-rs), [rspotify](https://github.com/ramsayleung/rspotify), [librespot](https://github.com/librespot-org/librespot), and [many more](https://github.com/aome510/spotify-player/blob/master/spotify_player/Cargo.toml). It's highly inspired by [spotify-tui](https://github.com/Rigellute/spotify-tui) and [ncspot](https://github.com/hrkfdn/ncspot).
- `spotify-player` can be used as either a remote player to control another Spotify client or a [local player](#streaming) with an integrated Spotify client. If you are familiar with other Spotify terminal applications, `spotify-player` can be viewed as a combination of [spotify-tui](https://github.com/Rigellute/spotify-tui) (remote player) and [ncspot](https://github.com/hrkfdn/ncspot) (local player).
- On startup, the application will connect to a running Spotify client. If there is no such client, user will need to use [Spotify connect](#spotify-connect) to connect to an available client.

## Examples

### Demo

A demo of `spotify-player` `v0.5.0-pre-release` on [youtube](https://www.youtube.com/shorts/Jbfe9GLNWbA) or on [asciicast](https://asciinema.org/a/446913):

[![asciicast](https://asciinema.org/a/446913.svg)](https://asciinema.org/a/446913)

### Playlist

![Playlist context example](https://user-images.githubusercontent.com/40011582/140253591-706d15d4-08c9-4527-997a-79fac73dee20.png)

### Artist

![Artist context example](https://user-images.githubusercontent.com/40011582/140253630-d958c5ea-23bc-4528-b40b-aa6fa68b5735.png)

### Album

![Album context example](https://user-images.githubusercontent.com/40011582/140253687-fd036da9-3b71-443b-a7f9-dad7721f01bf.png)

### Search

![Search page example](https://user-images.githubusercontent.com/40011582/140253653-5b156a8f-538b-4e68-9d52-0a379477574f.png)

## Installation

### Requirements

A Spotify Premium account is **required**.

To build and run the application, besides [Rust and cargo](https://www.rust-lang.org/tools/install) as the build requirements, Linux users will also need to install additional dependencies such as `openssl` and `alsa-lib`.

### Cargo

Run `cargo install spotify_player` to install the application from [crates.io](https://crates.io/crates/spotify_player).

### AUR

Run `yay -S spotify-player` to install the application as an AUR package.

### NetBSD

Using the package manager, run `pkgin install spotify-player` to install from the official repositories.

Building from source,

```
cd /usr/pkgsrc/audio/spotify-player
make install
```

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

## Spotify Connect

To enable full [Spotify connect](https://www.spotify.com/us/connect/) support, user will need to register a Spotify application and specify their own `client_id` in the application's general configuration file as described in the [configuration documentation](https://github.com/aome510/spotify-player/blob/master/doc/config.md#general).

More details on registering a Spotify application can be found in the [Spotify documentation](https://developer.spotify.com/documentation/general/guides/authorization/app-settings/).

If `spotify_player` runs with your own `client_id`, press **D** (default shortcut for `SwitchDevice` command) to get the list of available devices, then press **enter** (default shortcut for `ChooseSelected` command) to connect to the selected device.

An example of using Spotify connect to interact with Spotify's official client:

![Spotify Connect Example](https://user-images.githubusercontent.com/40011582/140323795-8a7ed2bb-7bda-4db2-9672-6036eac6e771.gif)

## Streaming

`spotify-player` supports streaming. It uses [librespot](https://github.com/librespot-org/librespot) library to create an integrated Spotify client while running. The integrated client will register a Spotify speaker device under the `spotify-player` name.

`spotify-player` uses [rodio](https://github.com/RustAudio/rodio) as the default [audio backend](https://github.com/librespot-org/librespot/wiki/Audio-Backends). List of available backends:

- `alsa-backend`
- `pulseaudio-backend`
- `rodio-backend`
- `portaudio-backend`
- `jackaudio-backend`
- `rodiojack-backend`
- `sdl-backend`
- `gstreamer-backend`

User can change the audio backend when building the application by specifying the `--features` option. For example, to build `spotify-player` with `pulseaudio-backend`, run

```shell
cargo build --release --no-default-features --features pulseaudio-backend
```

**Note**: user will need additional dependencies depending on the selected audio backend. More details can be found in the [Librespot documentation](https://github.com/librespot-org/librespot/wiki/Compiling#general-dependencies).

The `streaming` feature can be disabled by running (to use the application as a remote player only)

```shell
cargo build --release --no-default-features
```

## Commands

To open a shortcut help popup, press `?` or `C-h` (default shortcuts for `OpenCommandHelp` command).

List of supported commands:

| Command                     | Description                                                 | Default shortcuts  |
| --------------------------- | ----------------------------------------------------------- | ------------------ |
| `NextTrack`                 | next track                                                  | `n`                |
| `PreviousTrack`             | previous track                                              | `p`                |
| `ResumePause`               | resume/pause based on the current playback                  | `space`            |
| `PlayRandom`                | play a random track in the current context                  | `.`                |
| `Repeat`                    | cycle the repeat mode                                       | `C-r`              |
| `Shuffle`                   | toggle the shuffle mode                                     | `C-s`              |
| `VolumeUp`                  | increase playback volume by 5%                              | `+`                |
| `VolumeDown`                | decrease playback volume by 5%                              | `-`                |
| `Quit`                      | quit the application                                        | `C-c`, `q`         |
| `OpenCommandHelp`           | open a command help popup                                   | `?`, `C-h`         |
| `ClosePopup`                | close a popup                                               | `esc`              |
| `SelectNextOrScrollDown`    | select the next item in a list/table or scroll down         | `j`, `C-j`, `down` |
| `SelectPreviousOrScrollUp`  | select the previous item in a list/table or scroll up       | `k`, `C-k`, `up`   |
| `ChooseSelected`            | choose the selected item                                    | `enter`            |
| `RefreshPlayback`           | manually refresh the current playback                       | `r`                |
| `ReconnectIntegratedClient` | reconnect the integrated librespot client                   | `R`                |
| `ShowActionsOnSelectedItem` | open a popup showing actions on a selected item             | `g a`, `C-space`   |
| `ShowActionsOnCurrentTrack` | open a popup showing actions on the currently playing track | `a`                |
| `FocusNextWindow`           | focus the next focusable window (if any)                    | `tab`              |
| `FocusPreviousWindow`       | focus the previous focusable window (if any)                | `backtab`          |
| `SwitchTheme`               | open a popup for switching theme                            | `T`                |
| `SwitchDevice`              | open a popup for switching device                           | `D`                |
| `Search`                    | open a popup for searching in the current page              | `/`                |
| `BrowseUserPlaylists`       | open a popup for browsing user's playlists                  | `u p`              |
| `BrowseUserFollowedArtists` | open a popup for browsing user's followed artists           | `u a`              |
| `BrowseUserSavedAlbums`     | open a popup for browsing user's saved albums               | `u A`              |
| `BrowsePlayingContext`      | browse the current playing context                          | `g space`          |
| `LibraryPage`               | go to the user library page                                 | `g l`              |
| `SearchPage`                | go to the search page                                       | `g s`              |
| `PreviousPage`              | go to the previous page                                     | `backspace`, `C-p` |
| `SortTrackByTitle`          | sort the track table (if any) by track's title              | `s t`              |
| `SortTrackByArtists`        | sort the track table (if any) by track's artists            | `s a`              |
| `SortTrackByAlbum`          | sort the track table (if any) by track's album              | `s A`              |
| `SortTrackByDuration`       | sort the track table (if any) by track's duration           | `s d`              |
| `SortTrackByAddedDate`      | sort the track table (if any) by track's added date         | `s D`              |
| `ReverseOrder`              | reverse the order of the track table (if any)               | `s r`              |

To add new shortcuts or modify the default shortcuts, please refer to the [keymaps section](https://github.com/aome510/spotify-player/blob/master/doc/config.md#keymaps) in the configuration documentation.

### Actions

There will be a list of possible actions depending on the type of the corresponding Spotify item (track, album, artist, or playlist).
For example, the list of available actions on a track is `[BrowseAlbum, BrowseArtist, BrowseRecommandations, AddTrackToPlaylist, SaveToLibrary]`.

To get the list of actions on an item, call the `ShowActionsOnCurrentTrack` command or `ShowActionsOnSelectedItem` command.

### Search Page

When first entering the search page, the application focuses on the search input. User can then input text, delete one character backward using `backspace`, or search the text using `enter`.

To move the focus from the search input to the other windows such as track results, album results, etc, use `FocusNextWindow` or `FocusPreviousWindow`.

## Mouse support

Currently, the only use case of mouse is to seek to a position of the current playback by left-clicking to such position in the playback's progress bar.

## Configurations

By default, `spotify-player` will look into `$HOME/.config/spotify-player` for application's configuration files. This can be changed by either specifying `-c <FOLDER_PATH>` or `--config-folder <FOLDER_PATH>` option.

Please refer to [the configuration documentation](https://github.com/aome510/spotify-player/blob/master/doc/config.md) for more details on the configuration options.

## Caches

By default, `spotify-player` will look into `$HOME/.cache/spotify-player` for application's cache files, which include log file, spotify's authorization credentials, audio cache files, etc. This can be changed by either specifying `-C <FOLDER_PATH>` or `--cache-folder <FOLDER_PATH>` option.

### Logging

`spotify-player` uses `RUST_LOG` environment variable to define the application's [logging level](https://docs.rs/log/0.4.14/log/enum.Level.html) (default to be `INFO`). The application stores logs inside the `$APP_CACHE_FOLDER/spotify-player.log` file.

## Roadmap

- [x] integrate Spotify's [search APIs](https://developer.spotify.com/documentation/web-api/reference/#category-search)
- [x] integrate Spotify's [recommendation API](https://developer.spotify.com/console/get-recommendations/)
- [x] add supports for add track to playlist, save album, follow artist, and related commands.
- [ ] integrate Spotify's [recently played API](https://developer.spotify.com/console/get-recently-played/)
- [ ] handle networking error when running
- [x] add a (optional?) integrated spotify client (possibly use [librespot](https://github.com/librespot-org/librespot))
  - [ ] implement a custom connection logic to replace librespot's [spirc](https://github.com/librespot-org/librespot/blob/dev/connect/src/spirc.rs).
- [ ] add mpris (dbus) support
