# Community Theme Collections

This page showcases community-created theme collections for spotify-player. Instead of listing individual themes here, we maintain links to external repositories where you can browse and download theme collections.

## How to Use Community Themes

1. **Browse the collections**: Explore the theme repositories listed below
2. **Download themes**: Follow the installation instructions in each repository
3. **Add to your config**: Copy the theme definitions to your `$HOME/.config/spotify-player/theme.toml` file
4. **Set the theme**: Configure the theme in your `app.toml` or use the `-t/--theme` CLI flag
5. **Or use the theme switcher**: Press `T` in the application to open the theme switcher popup

For more details on theme configuration, see the [Configuration Documentation](./docs/config.md).

---

## Theme Collections

### [Spotify Player Themes](https://github.com/MBeggiato/spotify-player-theme-spotify)
**Author**: [@MBeggiato](https://github.com/MBeggiato)

**Description**: A comprehensive collection of 32 themes including recreations of popular music streaming services, popular terminal color schemes, retro & vintage themes, monochrome displays, atmospheric music themes, seasonal variations, and gaming-inspired designs.

**Themes**: 32 themes across 8 categories

**Categories**:
- Streaming Service Themes (5): Spotify, Apple Music, Tidal, YouTube Music, SoundCloud
- Popular Terminal Themes (8): Gruvbox, Dracula, Nord, Solarized Dark, One Dark, Monokai, Tokyo Night, Rose Pine
- Retro & Vintage Themes (4): MS-DOS, Windows 95, Vinyl Record, Cassette Tape
- Monochrome & Phosphor Themes (3): Amber Monochrome, Green Phosphor, Piano
- Music & Atmosphere Themes (4): Synthwave, Disco, Jazz Club, Rock Concert
- Seasonal Themes (4): Summer, Autumn, Winter, Spring
- Gaming & Pop Culture Themes (4): Cyberpunk 2077, Portal, Minecraft, Tetris


### [Catppuccin for spotify-player](https://github.com/catppuccin/spotify-player)
**Author**: [@catppuccin](https://github.com/catppuccin), [@elkrien](https://github.com/elkrien)

**Description**: 4 Catppuccin themes for spotify-player

**Themes**:
- 🌻 Latte
- 🪴 Frappé
- 🌺 Macchiato
- 🌿 Mocha

## Submit Your Theme Collection

Do you have a theme collection you'd like to share with the community? Submit a pull request to add your repository to this list!

**Submission Guidelines**:

- Create a GitHub repository for your theme collection
- Include clear installation instructions in your repository's README
- Organize themes in a logical structure (e.g., `theme.toml` with multiple theme definitions)
- Add screenshots for visual preview (recommended)
- Include a brief description and theme count
- Test themes to ensure they work correctly with spotify-player

**Pull Request Format**:

```markdown
### [Your Theme Collection Name](https://github.com/yourusername/your-repo)
**Author**: [@yourusername](https://github.com/yourusername)
**Description**: A brief description of your theme collection and what it offers.
**Themes**: [Number] themes across [categories/categories]
**Install**: [Installation command or instructions]
**Categories**: [Optional: List of theme categories]
```

---

## Creating Your Own Themes

If you want to create your own themes or theme collection:

1. **Start with examples**: Check `examples/theme.toml` for working theme definitions
2. **Use conversion tools**: Use the `scripts/theme_parse` utility to convert iTerm2 or Alacritty color schemes
3. **Test thoroughly**: Ensure good contrast and readability across all UI elements
4. **Document your inspiration**: Let others know if your theme is based on a popular color scheme
5. **Consider a repository**: If you create multiple themes, consider organizing them in a separate repository for easy sharing

### Quick Theme Template

```toml
[[themes]]
name = "Your Theme Name"

[themes.palette]
background = "#000000"
foreground = "#FFFFFF"
black = "#000000"
red = "#FF0000"
green = "#00FF00"
yellow = "#FFFF00"
blue = "#0000FF"
magenta = "#FF00FF"
cyan = "#00FFFF"
white = "#FFFFFF"
bright_black = "#808080"
bright_red = "#FF8080"
bright_green = "#80FF80"
bright_yellow = "#FFFF80"
bright_blue = "#8080FF"
bright_magenta = "#FF80FF"
bright_cyan = "#80FFFF"
bright_white = "#FFFFFF"

[themes.component_style]
# Optional: Override specific component styles
selection = { bg = "#333333" }
block_title = { fg = "#00FF00", modifiers = ["Bold"] }
```

---

*Have a theme collection to share? We'd love to feature it here!*
