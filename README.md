# spotify_player

## Table of Contents

- [Introduction](#introduction)
- [Examples](#examples)
  - [Demo](#demo)
- [Installation](#installation)
- [Features](#features)
  - [Spotify Connect](#spotify-connect)
  - [Streaming](#streaming)
  - [Lyric](#lyric)
  - [Media Control](#media-control)
  - [Image](#image)
  - [Notify](#notify)
  - [Mouse support](#mouse-support)
  - [Daemon](#daemon)
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
- Support [lyric](#lyric) for most songs.
- Support [cross-platform media control](#media-control).
- Support [image rendering](#image).
- Support [desktop notification](#notify).
- Support running the application as [a daemon](#daemon)
- Offer a wide range of [CLI commands](#cli-commands)

## Examples

### Demo

A demo of `spotify_player` `v0.5.0-pre-release` on [youtube](https://www.youtube.com/shorts/Jbfe9GLNWbA) or on [asciicast](https://asciinema.org/a/446913):

[![asciicast](https://asciinema.org/a/446913.svg)](https://asciinema.org/a/446913)

### Playlist page

![Playlist page example](https://user-images.githubusercontent.com/40011582/140253591-706d15d4-08c9-4527-997a-79fac73dee20.png)

### Artist page

![Artist page example](https://user-images.githubusercontent.com/40011582/140253630-d958c5ea-23bc-4528-b40b-aa6fa68b5735.png)

### Album page

![Album page example](https://user-images.githubusercontent.com/40011582/140253687-fd036da9-3b71-443b-a7f9-dad7721f01bf.png)

### Search page

![Search page example](https://user-images.githubusercontent.com/40011582/140253653-5b156a8f-538b-4e68-9d52-0a379477574f.png)

### Lyric page

![Lyric page example](https://user-images.githubusercontent.com/40011582/169437044-420cf0e2-5d75-4022-bd9f-34540f1fe230.png)

### Command help popup

![Command help popup example](https://user-images.githubusercontent.com/40011582/169437229-f5f1a3a5-d89e-4395-a416-6d45018f8971.png)

### Recommendation page

![Recommendation page example](https://user-images.githubusercontent.com/40011582/169440280-2f075ab1-04c3-419a-8614-0cad9c004d4f.gif)

## Installation

By default, the application's installed binary is `spotify_player`.

### Requirements

A Spotify Premium account is **required**.

#### Dependencies

##### Windows and MacOS

- [Rust and cargo](https://www.rust-lang.org/tools/install) as the build dependencies

##### Linux

- [Rust and cargo](https://www.rust-lang.org/tools/install) as the build dependencies
- `openssl`, `alsa-lib` (`streaming` feature), `libdbus` (`media-control` feature) system libraries.
  - On Debian based systems, run the below command to install application's dependencies:
    ```shell
    sudo apt install libssl-dev libasound2-dev libdbus-1-dev
    ```
  - On Fedora based systems, run the below command to install application's dependencies:
    ```shell
    sudo yum install openssl-devel alsa-lib-devel dbus-devel
    ```

### Binaries

Application's prebuilt binaries can be found in the [Releases Page](https://github.com/aome510/spotify-player/releases).

**Note**: to run the application, Linux systems need to install additional dependencies as specified in the [Dependencies section](#linux).

### Homebrew

Run `brew install spotify_player` to install the application.

### Cargo

Run `cargo install spotify_player` to install the application from [crates.io](https://crates.io/crates/spotify_player).

### AUR

Run `yay -S spotify-player` to install the application as an AUR package.

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

An example of using Spotify connect to interact with the Spotify's official application:

![Spotify Connect Example](https://user-images.githubusercontent.com/40011582/140323795-8a7ed2bb-7bda-4db2-9672-6036eac6e771.gif)

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

### Lyric

To enable lyric support, `spotify_player` needs to be built/installed with `lyric-finder` feature (**disabled** by default). To install the application with `lyric-finder` feature included run:

```shell
cargo install spotify_player --features lyric-finder
```

User can view lyric of the currently playing track by calling the `LyricPage` command to go the lyric page. To do this, `spotify_player` needs to be built with a `lyric-finder` feature.

Under the hood, `spotify_player` retrieves the song's lyric using [Genius.com](https://genius.com).

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

- `daemon` feature requires the `streaming` feature to be enabled and the application to be built with [an audio backend](#audio-backend)
- because of the OS's restrictions, `daemon` feature doesn't work with the `media-control` feature on Windows and MacOS, which is **enabled by default**. In other words, if you want to use the `daemon` feature on Windows or MacOS, you must install the application with `media-control` feature **disabled**:

  ```shell
  cargo install spotify_player --no-default-features --features daemon,rodio-backend
  ```

### CLI Commands

`spotify_player` offers several CLI commands to interact with **a running `spotify_player` instance**.

Under the hood, the application handles a CLI command by sending requests to a `spotify_player` instance's client socket running on the `client_port` port, a general application configuration with a default value `8080`.

Lists of CLI commands:

- `get`: Get Spotify data (playlist/album/artist data, user's data, etc)
- `playback`: Interact with the playback (start a playback, play-pause, next, etc)
- `connect`: Connect to a Spotify device
- `like`: Like currently playing track
- `authenticate`: Authenticate the application
- `playlist`: Playlist editing (new, delete, import, fork, etc)

For more details, run `spotify_player -h` or `spotify_player {command} -h`, in which `{command}` is a CLI command.

## Commands

To open a shortcut help popup, press `?` or `C-h` (default shortcuts for `OpenCommandHelp` command).

List of supported commands:

| Command                        | Description                                                             | Default shortcuts  |
| ------------------------------ | ----------------------------------------------------------------------- | ------------------ |
| `NextTrack`                    | next track                                                              | `n`                |
| `PreviousTrack`                | previous track                                                          | `p`                |
| `ResumePause`                  | resume/pause based on the current playback                              | `space`            |
| `PlayRandom`                   | play a random track in the current context                              | `.`                |
| `Repeat`                       | cycle the repeat mode                                                   | `C-r`              |
| `Shuffle`                      | toggle the shuffle mode                                                 | `C-s`              |
| `VolumeUp`                     | increase playback volume by 5%                                          | `+`                |
| `VolumeDown`                   | decrease playback volume by 5%                                          | `-`                |
| `SeekForward`                  | seek forward by 5s                                                      | `>`                |
| `SeekBackward`                 | seek backward by 5s                                                     | `<`                |
| `Quit`                         | quit the application                                                    | `C-c`, `q`         |
| `OpenCommandHelp`              | open a command help popup                                               | `?`, `C-h`         |
| `ClosePopup`                   | close a popup                                                           | `esc`              |
| `SelectNextOrScrollDown`       | select the next item in a list/table or scroll down                     | `j`, `C-n`, `down` |
| `SelectPreviousOrScrollUp`     | select the previous item in a list/table or scroll up                   | `k`, `C-p`, `up`   |
| `PageSelectNextOrScrollDown`   | select the next page item in a list/table or scroll a page down         | `page_down`, `C-f` |
| `PageSelectPreviousOrScrollUp` | select the previous page item in a list/table or scroll a page up       | `page_up`, `C-b`   |
| `SelectFirstOrScrollToTop`     | select the first item in a list/table or scroll to the top              | `g g`, `home`      |
| `SelectLastOrScrollToBottom`   | select the last item in a list/table or scroll to the bottom            | `G`, `end`         |
| `ChooseSelected`               | choose the selected item                                                | `enter`            |
| `RefreshPlayback`              | manually refresh the current playback                                   | `r`                |
| `RestartIntegratedClient`      | restart the integrated librespot client (`streaming` feature only)      | `R`                |
| `ShowActionsOnSelectedItem`    | open a popup showing actions on a selected item                         | `g a`, `C-space`   |
| `ShowActionsOnCurrentTrack`    | open a popup showing actions on the current track                       | `a`                |
| `AddSelectedItemToQueue`       | add the selected item to queue                                          | `Z`                |
| `FocusNextWindow`              | focus the next focusable window (if any)                                | `tab`              |
| `FocusPreviousWindow`          | focus the previous focusable window (if any)                            | `backtab`          |
| `SwitchTheme`                  | open a popup for switching theme                                        | `T`                |
| `SwitchDevice`                 | open a popup for switching device                                       | `D`                |
| `Search`                       | open a popup for searching in the current page                          | `/`                |
| `Queue`                        | open a popup for showing the current queue                              | `z`                |
| `BrowseUserPlaylists`          | open a popup for browsing user's playlists                              | `u p`              |
| `BrowseUserFollowedArtists`    | open a popup for browsing user's followed artists                       | `u a`              |
| `BrowseUserSavedAlbums`        | open a popup for browsing user's saved albums                           | `u A`              |
| `CurrentlyPlayingContextPage`  | go to the currently playing context page                                | `g space`          |
| `TopTrackPage`                 | go to the user top track page                                           | `g t`              |
| `RecentlyPlayedTrackPage`      | go to the user recently played track page                               | `g r`              |
| `LikedTrackPage`               | go to the user liked track page                                         | `g y`              |
| `LyricPage`                    | go to the lyric page of the current track (`lyric-finder` feature only) | `g L`, `l`         |
| `LibraryPage`                  | go to the user library page                                             | `g l`              |
| `SearchPage`                   | go to the search page                                                   | `g s`              |
| `BrowsePage`                   | go to the browse page                                                   | `g b`              |
| `PreviousPage`                 | go to the previous page                                                 | `backspace`, `C-q` |
| `SortTrackByTitle`             | sort the track table (if any) by track's title                          | `s t`              |
| `SortTrackByArtists`           | sort the track table (if any) by track's artists                        | `s a`              |
| `SortTrackByAlbum`             | sort the track table (if any) by track's album                          | `s A`              |
| `SortTrackByDuration`          | sort the track table (if any) by track's duration                       | `s d`              |
| `SortTrackByAddedDate`         | sort the track table (if any) by track's added date                     | `s D`              |
| `ReverseOrder`                 | reverse the order of the track table (if any)                           | `s r`              |
| `MovePlaylistItemUp`           | move playlist item up one position                                      | `C-k`              |
| `MovePlaylistItemDown`         | move playlist item down one position                                    | `C-j`              |

To add new shortcuts or modify the default shortcuts, please refer to the [keymaps section](docs/config.md#keymaps) in the configuration documentation.

**Tips**:

- `RefreshPlayback` can be used to manually update the playback status.
- `RestartIntegratedClient` is useful when user wants to switch to another audio device (headphone, earphone, etc) without restarting the application, as the integrated client will be re-initialized with the new device.

### Actions

A list of actions is available for each type of Spotify item (track, album, artist, or playlist).
For example, the list of available actions on a track is `[GoToAlbum, GoToArtist, GoToTrackRadio, GoToArtistRadio, GoToAlbumRadio, AddToPlaylist, DeleteFromCurrentPlaylist, AddToLikedTracks, DeleteFromLikedTracks]`.

To get the list of actions on an item, call the `ShowActionsOnCurrentTrack` command or `ShowActionsOnSelectedItem` command, then press enter (default binding for `ChooseSelected` command) to initiate the selected action.

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

`spotify_player` is written in [Rust](https://www.rust-lang.org) and is built on top of awesome libraries such as [tui-rs](https://github.com/fdehau/tui-rs), [rspotify](https://github.com/ramsayleung/rspotify), [librespot](https://github.com/librespot-org/librespot), and [many more](spotify_player/Cargo.toml). It's highly inspired by [spotify-tui](https://github.com/Rigellute/spotify-tui) and [ncspot](https://github.com/hrkfdn/ncspot).
