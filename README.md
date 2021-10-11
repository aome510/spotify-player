# spotify-player

## Table of Contents

- [Introduction](#introduction)
  - [Requirements](#requirements)
  - [Spotify Connect](#spotify-connect)
  - [Streaming](#streaming)
- [Installation](#installation)
  - [Cargo](#cargo)
  - [AUR](#aur)
  - [NetBSD](#netbsd)
  - [Docker](#docker)
- [Examples](#examples)
  - [Demo](#demo)
  - [Playlist](#playlist)
  - [Artist](#artist)
  - [Album](#album)
  - [Search](#search)
- [Commands](#commands)
  - [Search Page](#search-page)
- [Mouse support](#mouse-support)
- [Configurations](#configurations)
- [Roadmap](#roadmap)

## Introduction

- `spotify-player` is a fast, easy to use, and [configurable](https://github.com/aome510/spotify-player/blob/master/doc/config.md) Spotify player.
- `spotify-player` is designed to be a player, not a fully-fledged Spotify clone, so it does not aim to support all possible Spotify features. Its main goal is to provide a quick and intuitive way to control music using [commands](#commands).
- `spotify-player` is built on top of [tui](https://github.com/fdehau/tui-rs), [rspotify](https://github.com/ramsayleung/rspotify), and [librespot](https://github.com/librespot-org/librespot) libraries. It's inspired by [spotify-tui](https://github.com/Rigellute/spotify-tui) and [ncspot](https://github.com/hrkfdn/ncspot).
- `spotify-player` can be used as either a remote player to control a running Spotify client or a [local player](#streaming) with an integrated Spotify client. On startup, the application will connect to the currently running Spotify client. If not exist such client, user will need to use [Spotify connect](#spotify-connect) to connect to an available client.

### Requirements

User will need to have a Spotify Premium account to use all application's supported features.

### Spotify Connect

To enable [Spotify connect](https://www.spotify.com/us/connect/) support, user will need to register a Spotify application and specify their own `client_id` in the application's general configuration file as described in the [configuration documentation](https://github.com/aome510/spotify-player/blob/master/doc/config.md#general).

More details on registering a Spotify application can be found in the [Spotify documentation](https://developer.spotify.com/documentation/general/guides/app-settings/).

**Note**: when using the default value for `client_id`, `spotify-player` can still be used as a remote player but it requires to have a running Spotify client before starting the application.

When `spotify_player` runs with your own `client_id`, press **D** (default shortcut for `SwitchDevice` command) to get the list of available devices then press **enter** (default shortcut for `ChooseSelected` command) to connect to the selected device.

### Streaming

`spotify-player` supports streaming by using [librespot](https://github.com/librespot-org/librespot) library to create an integrated Spotify client while running. User will need to use their own `client_id` to connect to the integrated client as described in the [Spotify Connect](#spotify-connect) section. By default, the integrated client will create a Spotify device under `spotify-player` name.

**Note:** using an integrated client can result in a slow startup time to connect to the player and retrieve the playback data. I'm still investigating on how to improve the startup time.

The integrated client will use [rodio](https://github.com/RustAudio/rodio) as the default [audio backend](https://github.com/librespot-org/librespot/wiki/Audio-Backends). List of available backends:

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

**Note**: user will need additional dependencies depending on the selected audio backend. More details on compiling can be found in the [Librespot documentation](https://github.com/librespot-org/librespot/wiki/Compiling#general-dependencies).

User can also disable the `streaming` feature by running

```shell
cargo build --release --no-default-features
```

## Installation

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

**Note**: [streaming](#streaming) feature is disabled when using docker image.

You can download the binary image of the latest build from the `master` branch by running

```
docker pull aome510/spotify_player:latest
```

then run

```
docker run --rm -it aome510/spotify_player:latest
```

to run the application.

You can also use your local config folder to configure the application or your local cache folder to store the authentication token when running the docker image:

```
docker run --rm \
-v $APP_CONFIG_FOLDER:/app/config/ \
-v $APP_CACHE_FOLDER:/app/cache/ \
-it aome510/spotify_player:latest
```

## Examples

### Demo

A demo of `spotify-player v0.1.0`:

[![asciicast](https://asciinema.org/a/430335.svg)](https://asciinema.org/a/430335)

### Playlist

![Playlist context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/playlist.png)

### Artist

![Artist context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/artist.png)

### Album

![Album context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/album.png)

### Search

![Search page example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/search.png)

## Commands

To open a command shortcut help popup when running the application, press `?` or `C-h` (default shortcuts for `OpenCommandHelp` command).

List of supported commands:

| Command                     | Description                                               | Default shortcuts  |
| --------------------------- | --------------------------------------------------------- | ------------------ |
| `NextTrack`                 | next track                                                | `n`                |
| `PreviousTrack`             | previous track                                            | `p`                |
| `ResumePause`               | resume/pause based on the current playback                | `space`            |
| `PlayContext`               | play a random track in the current context                | `.`                |
| `Repeat`                    | cycle the repeat mode                                     | `C-r`              |
| `Shuffle`                   | toggle the shuffle mode                                   | `C-s`              |
| `VolumeUp`                  | increase playback volume                                  | `+`                |
| `VolumeDown`                | decrease playback volume                                  | `-`                |
| `Quit`                      | quit the application                                      | `C-c`, `q`         |
| `OpenCommandHelp`           | open a command help popup                                 | `?`, `C-h`         |
| `ClosePopup`                | close a popup                                             | `esc`              |
| `SelectNextOrScrollDown`    | select the next item in a list/table or scroll down       | `j`, `C-j`, `down` |
| `SelectPreviousOrScrollUp`  | select the previous item in a list/table or scroll up     | `k`, `C-k`, `up`   |
| `ChooseSelected`            | choose the selected item and act on it                    | `enter`            |
| `RefreshPlayback`           | manually refresh the current playback                     | `r`                |
| `ShowActionsOnSelectedItem` | show actions on a selected item                           | `g a`, `C-space`   |
| `FocusNextWindow`           | focus the next focusable window (if any)                  | `tab`              |
| `FocusPreviousWindow`       | focus the previous focusable window (if any)              | `backtab`          |
| `SwitchTheme`               | open a popup for switching theme                          | `T`                |
| `SwitchDevice`              | open a popup for switching device                         | `D`                |
| `SearchContext`             | open a popup for searching the current context            | `/`                |
| `BrowseUserPlaylists`       | open a popup for browsing user's playlists                | `u p`              |
| `BrowseUserFollowedArtists` | open a popup for browsing user's followed artists         | `u a`              |
| `BrowseUserSavedAlbums`     | open a popup for browsing user's saved albums             | `u A`              |
| `BrowsePlayingTrackArtists` | open a popup for browsing current playing track's artists | `a`                |
| `BrowsePlayingTrackAlbum`   | browse the current playing track's album                  | `A`                |
| `BrowsePlayingContext`      | browse the current playing context                        | `g space`          |
| `SearchPage`                | go to the search page                                     | `g s`              |
| `PreviousPage`              | go to the previous page                                   | `backspace`, `C-p` |
| `SortTrackByTitle`          | sort the track table (if any) by track's title            | `s t`              |
| `SortTrackByArtists`        | sort the track table (if any) by track's artists          | `s a`              |
| `SortTrackByAlbum`          | sort the track table (if any) by track's album            | `s A`              |
| `SortTrackByDuration`       | sort the track table (if any) by track's duration         | `s d`              |
| `SortTrackByAddedDate`      | sort the track table (if any) by track's added date       | `s D`              |
| `ReverseOrder`              | reverse the order of the track table (if any)             | `s r`              |

To add new shortcuts or modify the default shortcuts, please refer to the [keymaps section](https://github.com/aome510/spotify-player/blob/master/doc/config.md#keymaps) in the configuration documentation.

### Search Page

When first entering the search page, the application places a focus on the search input. User can input text, delete one character backward using `backspace`, or search the text using `enter`.

To move the focus from the search input to the other windows such as track results, album results, etc, use `FocusNextWindow` or `FocusPreviousWindow`.

## Mouse support

Currently, the only use case of mouse is to seek to a position of the current playback by left-clicking to such position in the playback's progress bar.

## Configurations

By default, `spotify-player` will look into `$HOME/.config/spotify-player` for application's configuration files. This can be changed by either specifying `-c <FOLDER_PATH>` or `--config-folder <FOLDER_PATH>` option.

Please refer to [the configuration documentation](https://github.com/aome510/spotify-player/blob/master/doc/config.md) for more details on the configuration options.

## Roadmap

- [x] integrate Spotify's [search APIs](https://developer.spotify.com/documentation/web-api/reference/#category-search)
- [ ] integrate Spotify's [recommendation API](https://developer.spotify.com/console/get-recommendations/)
- [x] add supports for add track to playlist, save album, follow artist, and related commands.
- [ ] integrate Spotify's [recently played API](https://developer.spotify.com/console/get-recently-played/)
- [ ] handle networking error when running
- [x] add a (optional?) integrated spotify client (possibly use [librespot](https://github.com/librespot-org/librespot))
  - [ ] implement a custom connection logic to replace librespot's [spirc](https://github.com/librespot-org/librespot/blob/dev/connect/src/spirc.rs).
- [ ] add mpris (dbus) support
