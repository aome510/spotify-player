# spotify-player

## Table of Contents

- [Introduction](#introduction)
- [Prerequisites](#prerequisites)
  - [Setup](#setup)
- [Installation](#installation)
  - [Arch Linux AUR](#arch-linux-aur)
  - [Cargo](#cargo)
  - [Docker](#docker)
- [Examples](#examples)
  - [Demo](#demo)
  - [Playlist](#playlist)
  - [Artist](#artist)
  - [Album](#album)
- [Command table](#commands)
- [Mouse support](#mouse-support)
- [Configurations](#configurations)
- [Roadmap](#roadmap)

## Introduction

- `spotify-player` is a custom Spotify player that I built and tweaked based on my personal preferences. It is fast, easy to use, and [configurable](https://github.com/aome510/spotify-player/blob/master/doc/config.md).
- `spotify-player` is designed to be a player, not a fully-fledged Spotify clone, so it does not aim to support all possible Spotify features. Its main goal is to provide a quick and intuitive way to modify the current playback by either using player commands or navigating between different contexts.
- `spotify-player`, similar to other TUI applications, is keyboard driven. User will use a set of [predefined commands with shortcuts](#commands) to interact with the player.
- `spotify-player` has a simple UI with three main components:
  - a playback window displaying the current playback
  - a context window displaying a context (playlist, album, artist)
  - popup windows for using some commands (switch theme, browser playlists, etc) or displaying additional information
- `spotify-player` is built on top of [tui](https://github.com/fdehau/tui-rs) and [rspotify](https://github.com/ramsayleung/rspotify) libraries. It's also inspired by [spotify-tui](https://github.com/Rigellute/spotify-tui).

## Prerequisites

- A running Spotify client (official Spotify application or [third-party clients](https://wiki.archlinux.org/title/Spotify#Third-party_clients))
- A premium Spotify account for full functionalities

### Setup

- Create a configuration folder to store application's configuration files and authentication token cache. By default, the application will look into `$HOME/.config/spotify-player`. You can specify another path by adding the `-c <FOLDER_PATH>` option.
- Follow the steps described in [Spotify documentation](https://developer.spotify.com/documentation/general/guides/app-settings/) to register an application with `client_id` and `client_secret` as well as to whitelist a redirect URI.
- For the redirect URI, specify `http://localhost:8888/callback`.
- Create a new `client.toml` file in the application's configuration folder with `client_id` and `client_secret` entries as follow

  ```toml
  client_id = ${APP CLIENT ID}
  client_secret = ${APP CLIENT SECRET}
  ```

- When running the application for the first time, you will be directed to an official Spotify page that asks for the application's permissions. If you run the application using [docker](#docker), you will need to open the page in browser by yourself.

![Callback docker example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/callback_docker.png)

- After accepting the permissions, you will be redirected to a URL as follows `localhost:8888/callback?code=AQAn75sPSJIg...`. Copy the URL and paste it into the terminal prompt, then the application should run successfully, given that there is a running Spotify client:

![Callback example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/callback.png)

## Installation

Before running the application, make sure you have read the [setup instructions](#setup).

### Arch Linux AUR

Run `yay -S spotify-player` to install the application as an AUR package.

### Cargo

Run `cargo install spotify_player` to install the application from [crates.io](https://crates.io/crates/spotify_player).

### Docker

You can download the binary image of the latest build from the `master` branch by running

```
# docker pull aome510/spotify_player:latest
```

then run

```
docker run --rm -v $APP_CONFIG_FOLDER_PATH:/app/config/ -it aome510/spotify_player:latest
```

with `$APP_CONFIG_FOLDER_PATH` is the application's configuration folder.

## Examples

### Demo

[![asciicast](https://asciinema.org/a/430335.svg)](https://asciinema.org/a/430335)

### Playlist

![Playlist context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/playlist.png)

### Artist

![Artist context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/artist.png)

### Album

![Album context example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/album.png)

## Commands

To open a command shortcut help popup when running the application, press `?` or `C-h` (default shortcuts for `OpenCommandHelp` command).

List of supported commands:

| Command                      | Description                                               | Default shortcuts  |
| ---------------------------- | --------------------------------------------------------- | ------------------ |
| `NextTrack`                  | next track                                                | `n`                |
| `PreviousTrack`              | previous track                                            | `p`                |
| `ResumePause`                | resume/pause based on the current playback                | `space`            |
| `PlayContext`                | play a random track in the current context                | `.`                |
| `Repeat`                     | cycle the repeat mode                                     | `C-r`              |
| `Shuffle`                    | toggle the shuffle mode                                   | `C-s`              |
| `Quit`                       | quit the application                                      | `C-c`, `q`         |
| `OpenCommandHelp`            | open a command help popup                                 | `?`, `C-h`         |
| `ClosePopup`                 | close a popup                                             | `esc`              |
| `SelectNext`                 | select the next item in the focused list or table         | `j`, `C-j`, `down` |
| `SelectPrevious`             | select the previous item in the focused list or table     | `k`, `C-k`, `up`   |
| `ChooseSelected`             | choose the selected item and act on it                    | `enter`            |
| `RefreshPlayback`            | manually refresh the current playback                     | `r`                |
| `FocusNextWindow`            | focus the next focusable window (if any)                  | `tab`              |
| `FocusPreviousWindow`        | focus the previous focusable window (if any)              | `backtab`          |
| `SwitchTheme`                | open a popup for switching theme                          | `T`                |
| `SwitchDevice`               | open a popup for switching device                         | `D`                |
| `SearchContext`              | open a popup for searching the current context            | `/`                |
| `BrowseUserPlaylists`        | open a popup for browsing user's playlists                | `u p`              |
| `BrowseUserFollowedArtists`  | open a popup for browsing user's followed artists         | `u a`              |
| `BrowseUserSavedAlbums`      | open a popup for browsing user's saved albums             | `u A`              |
| `BrowsePlayingTrackArtists`  | open a popup for browsing current playing track's artists | `a`                |
| `BrowsePlayingTrackAlbum`    | browse the current playing track's album                  | `A`                |
| `BrowsePlayingContext`       | browse the current playing context                        | `g space`          |
| `BrowseSelectedTrackArtists` | open a popup for browsing the selected track's artists    | `g a`, `C-g a`     |
| `BrowseSelectedTrackAlbum`   | browse to the selected track's album                      | `g A`, `C-g A`     |
| `PreviousPage`               | go to the previous page                                   | `backspace`        |
| `SortTrackByTitle`           | sort the track table (if any) by track's title            | `s t`              |
| `SortTrackByArtists`         | sort the track table (if any) by track's artists          | `s a`              |
| `SortTrackByAlbum`           | sort the track table (if any) by track's album            | `s A`              |
| `SortTrackByDuration`        | sort the track table (if any) by track's duration         | `s d`              |
| `SortTrackByAddedDate`       | sort the track table (if any) by track's added date       | `s D`              |
| `ReverseOrder`               | reverse the order of the track table (if any)             | `s r`              |

## Mouse support

Currently, the only use case of mouse is to seek to a position of the current playback by left-clicking to such position in the playback's progress bar.

## Configurations

By default, `spotify-player` will look into `$HOME/.config/spotify-player` for application's configuration files. This can be changed by either specifying `-c <FOLDER_PATH>` or `--config-folder <FOLDER_PATH>` option.

Please refer to [the configuration documentation](https://github.com/aome510/spotify-player/blob/master/doc/config.md) for more details on the configuration options.

## Roadmap

- [ ] integrating Spotify's [search APIs](https://developer.spotify.com/documentation/web-api/reference/#category-search)
