# spotify-player

My custom Spotify Player

## Table of Contents

- [Installation](#installation)
- [Examples](#examples)
  - [Demo](#demo)
  - [Playlist](#playlist)
  - [Artist](#artist)
  - [Album](#album)
- [Command table](#commands)
- [Mouse support](#mouse-support)
- [Roadmap](#roadmap)

## Installation

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

List of supported commands:

| Command                     | Description                                                        | Default shortcuts  |
| --------------------------- | ------------------------------------------------------------------ | ------------------ |
| `NextTrack`                 | next track                                                         | `n`                |
| `PreviousTrack`             | previous track                                                     | `p`                |
| `ResumePause`               | resume/pause based on the playback                                 | `space`            |
| `PlayContext`               | play a random track in the current context                         | `.`                |
| `Repeat`                    | cycle the repeat mode                                              | `C-r`              |
| `Shuffle`                   | toggle the shuffle mode                                            | `C-s`              |
| `Quit`                      | quit the application                                               | `C-c`, `q`         |
| `OpenCommandHelp`           | open the help a command help popup                                 | `?`                |
| `ClosePopup`                | close a popup                                                      | `esc`              |
| `SelectNext`                | select the next item in the focused list or a table                | `j`, `C-j`, `down` |
| `SelectPrevious`            | select the previous item in the focused list or a table            | `k`, `C-k`, `up`   |
| `ChooseSelected`            | choose the selected item and act on it                             | `enter`            |
| `RefreshPlayback`           | refresh the current playback                                       | `r`                |
| `FocusNextWindow`           | focus the next focusable window (if any)                           | `tab`              |
| `FocusPreviousWindow`       | focus the previous focusable window (if any)                       | `backtab`          |
| `SwitchTheme`               | open a theme switch popup                                          | `T`                |
| `SwitchDevice`              | open a device switch popup                                         | `D`                |
| `SearchContext`             | open a search popup for searching in context                       | `/`                |
| `BrowseUserPlaylist`        | open a playlist popup for browsing user's playlists                | `g p`              |
| `BrowsePlayingContext`      | browse the current playing context                                 | `g space`          |
| `BrowsePlayingTrackArtist`  | open an artist popup for browsing current playing track's artists  | `g a`              |
| `BrowsePlayingTrackAlbum`   | browse to the current playing track's album page                   | `g A`              |
| `BrowseSelectedTrackArtist` | open an artist popup for browsing current selected track's artists | `g s a`, `C-g a`   |
| `BrowseSelectedTrackAlbum`  | browse to the current selected track's album page                  | `g s A`, `C-g A`   |
| `PreviousPage`              | go to the previous page                                            | `backspace`        |
| `SortTrackByTitle`          | sort the track table (if any) by track's title                     | `s t`              |
| `SortTrackByArtists`        | sort the track table (if any) by track's artists                   | `s a`              |
| `SortTrackByAlbum`          | sort the track table (if any) by track's album                     | `s A`              |
| `SortTrackByDuration`       | sort the track table (if any) by track's duration                  | `s d`              |
| `SortTrackByAddedDate`      | sort the track table (if any) by track's added date                | `s D`              |
| `ReverseOrder`              | reverse the order of the track table (if any)                      | `s r`              |

## Mouse support

Currently, the only use case of mouse is to seek to a position of the current playback by right-clicking to such position in the playback's progress bar.

## Roadmap
