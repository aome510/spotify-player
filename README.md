# spotify-player

## Table of Contents

- [Introduction](#introduction)
- [Prerequisites](#prerequisites)
  - [Setup](#setup)
- [Installation](#installation)
- [Examples](#examples)
  - [Demo](#demo)
  - [Playlist](#playlist)
  - [Artist](#artist)
  - [Album](#album)
- [Command table](#commands)
- [Mouse support](#mouse-support)
- [Roadmap](#roadmap)

## Introduction

- `spotify-player` is a custom Spotify player that I built and tweaked based on my personal preferences. It is fast, easy to use, and [configurable](https://github.com/aome510/spotify-player/blob/master/doc/config.md).
- `spotify-player` is designed to be a player, not a fully-fledged Spotify application, so it does not aim to support all possible Spotify features. Its main goal is to provide an easy and intuitive way to quickly change your current playback.
- `spotify-player`, similar to other TUI applications, is keyboard driven, so user will interact with the application using a set of [commands with shortcuts](#commands).
- `spotify-player` has a simple UI with three main components:
  - a playback window displaying the current playing track's data
  - a context window displaying a playing context's data (playlist, album, artist)
  - popup windows for using some commands (switch theme, browser playlists, etc) or displaying additional information

## Prerequisites

- A running Spotify client (official Spotify application or [third-party clients](https://wiki.archlinux.org/title/Spotify#Third-party_clients))
- A premium Spotify account for full functionalities

### Setup

- Create a configuration folder to store application's configuration files and authentication token cache. By default, the application will look into `$HOME/.config/spotify-player`. You can specify another path by adding the `-c <FOLDER_PATH>` option.
- Follow the steps described in [Spotify documentation](https://developer.spotify.com/documentation/general/guides/app-settings/) to register an application with `client_id` and `client_secret` as well as to whitelist the application's redirect URI.
- For the redirect URI, specify `http://localhost:8888/callback`.
- Create a new `client.toml` file in the application's configuration folder with `client_id` and `client_secret` entries as follow

  ```toml
  client_id = ${APP CLIENT ID}
  client_secret = ${APP CLIENT SECRET}
  ```

- When running the application for the first time, you will be directed to an official Spotify page that asks for the application's permissions. If you run the application using [docker](#docker), you will need to open the Spotify page in browser by yourself.

![Callback docker example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/callback_docker.png)

- After accepting the permissions, you will be redirected to a URL as follows `localhost:8888/callback?code=AQAn75sPSJIg...`. Copy the URL then paste it into the terminal prompt, then the application should be running given that there exists a Spotify client running:

![Callback example](https://raw.githubusercontent.com/aome510/spotify-player/master/examples/callback.png)

## Installation

Before following those below steps, please read the [setup instructions](#setup) first.

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

| Command                     | Description                                                | Default shortcuts  |
| --------------------------- | ---------------------------------------------------------- | ------------------ |
| `NextTrack`                 | next track                                                 | `n`                |
| `PreviousTrack`             | previous track                                             | `p`                |
| `ResumePause`               | resume/pause based on the playback                         | `space`            |
| `PlayContext`               | play a random track in the current context                 | `.`                |
| `Repeat`                    | cycle the repeat mode                                      | `C-r`              |
| `Shuffle`                   | toggle the shuffle mode                                    | `C-s`              |
| `Quit`                      | quit the application                                       | `C-c`, `q`         |
| `OpenCommandHelp`           | open the help a command help popup                         | `?`, `C-h`         |
| `ClosePopup`                | close a popup                                              | `esc`              |
| `SelectNext`                | select the next item in the focused list or a table        | `j`, `C-j`, `down` |
| `SelectPrevious`            | select the previous item in the focused list or a table    | `k`, `C-k`, `up`   |
| `ChooseSelected`            | choose the selected item and act on it                     | `enter`            |
| `RefreshPlayback`           | refresh the current playback                               | `r`                |
| `FocusNextWindow`           | focus the next focusable window (if any)                   | `tab`              |
| `FocusPreviousWindow`       | focus the previous focusable window (if any)               | `backtab`          |
| `SwitchTheme`               | open a popup for switching theme                           | `T`                |
| `SwitchDevice`              | open a popup for switching device                          | `D`                |
| `SearchContext`             | open a search popup for searching in context               | `/`                |
| `BrowseUserPlaylist`        | open a popup for browsing user's playlists                 | `u p`              |
| `BrowseUserSavedArtists`    | "open a popup for browsing user's saved artists            | `u a`              |
| `BrowseUserSavedAlbums`     | open a popup for browsing user's saved albums              | `u A`              |
| `BrowsePlayingTrackArtist`  | open a popup for browsing current playing track's artists  | `a`                |
| `BrowsePlayingTrackAlbum`   | browse to the current playing track's album page           | `A`                |
| `BrowsePlayingContext`      | browse the current playing context                         | `g space`          |
| `BrowseSelectedTrackArtist` | open a popup for browsing current selected track's artists | `g a`, `C-g a`     |
| `BrowseSelectedTrackAlbum`  | browse to the current selected track's album page          | `g A`, `C-g A`     |
| `PreviousPage`              | go to the previous page                                    | `backspace`        |
| `SortTrackByTitle`          | sort the track table (if any) by track's title             | `s t`              |
| `SortTrackByArtists`        | sort the track table (if any) by track's artists           | `s a`              |
| `SortTrackByAlbum`          | sort the track table (if any) by track's album             | `s A`              |
| `SortTrackByDuration`       | sort the track table (if any) by track's duration          | `s d`              |
| `SortTrackByAddedDate`      | sort the track table (if any) by track's added date        | `s D`              |
| `ReverseOrder`              | reverse the order of the track table (if any)              | `s r`              |

## Mouse support

Currently, the only use case of mouse is to seek to a position of the current playback by right-clicking to such position in the playback's progress bar.

## Roadmap

- [ ] integrating Spotify's [search APIs](https://developer.spotify.com/documentation/web-api/reference/#category-search)
