use std::{io::Write, sync::OnceLock};

use anyhow::Result;

use crate::config::Command;

static CLIPBOARD_PROVIDER: OnceLock<Box<dyn ClipboardProvider>> = OnceLock::new();

pub trait ClipboardProvider: Send + Sync {
    fn get_contents(&self) -> Result<String>;
    fn set_contents(&self, contents: String) -> Result<()>;
}

struct CommandProvider {
    copy_command: Command,
    paste_command: Command,
}

#[cfg(not(target_os = "windows"))]
struct NopProvider {}

#[cfg(target_os = "windows")]
struct WindowsProvider {}

impl ClipboardProvider for CommandProvider {
    fn get_contents(&self) -> Result<String> {
        let output = std::process::Command::new(&self.paste_command.command)
            .args(&self.paste_command.args)
            .output()?;
        Ok(String::from_utf8(output.stdout)?)
    }

    fn set_contents(&self, contents: String) -> Result<()> {
        let mut child = std::process::Command::new(&self.copy_command.command)
            .args(&self.copy_command.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(contents.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("copy command failed");
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl ClipboardProvider for NopProvider {
    fn get_contents(&self) -> Result<String> {
        anyhow::bail!("no clipboard provider found!")
    }
    fn set_contents(&self, _contents: String) -> Result<()> {
        anyhow::bail!("no clipboard provider found!")
    }
}

#[cfg(target_os = "windows")]
impl ClipboardProvider for WindowsProvider {
    fn get_contents(&self) -> Result<String> {
        let contents = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .map_err(|_| anyhow::anyhow!("failed to get windows clipboard"))?;
        Ok(contents)
    }
    fn set_contents(&self, contents: String) -> Result<()> {
        clipboard_win::set_clipboard(clipboard_win::formats::Unicode, contents)
            .map_err(|_| anyhow::anyhow!("failed to set windows clipboard"))?;
        Ok(())
    }
}

/// Get a clipboard provider based on user's environment
// The function's implementation is inspired by helix
// (https://github.com/blaggacao/helix/blob/master/helix-view/src/clipboard.rs)
pub fn get_clipboard_provider() -> Box<dyn ClipboardProvider> {
    if binary_exists("pbcopy") && binary_exists("pbpaste") {
        Box::new(CommandProvider {
            paste_command: Command::new::<_, &str>("pbpaste", &[]),
            copy_command: Command::new::<_, &str>("pbcopy", &[]),
        })
    } else if env_var_is_set("WAYLAND_DISPLAY")
        && binary_exists("wl-copy")
        && binary_exists("wl-paste")
    {
        Box::new(CommandProvider {
            paste_command: Command::new("wl-paste", &["--no-newline"]),
            copy_command: Command::new("wl-copy", &["--type", "text/plain"]),
        })
    } else if env_var_is_set("DISPLAY") && binary_exists("xclip") {
        Box::new(CommandProvider {
            paste_command: Command::new("xclip", &["-o", "-selection", "clipboard"]),
            copy_command: Command::new("xclip", &["-i", "-selection", "clipboard"]),
        })
    } else if env_var_is_set("DISPLAY") && binary_exists("xsel") {
        Box::new(CommandProvider {
            paste_command: Command::new("xsel", &["-o", "-b"]),
            copy_command: Command::new("xsel", &["--nodetach", "-i", "-b"]),
        })
    } else {
        #[cfg(target_os = "windows")]
        return Box::new(WindowsProvider {});

        #[cfg(not(target_os = "windows"))]
        {
            tracing::warn!("No clipboard provider found! Fallback to a NOP clipboard provider.");
            Box::new(NopProvider {})
        }
    }
}

fn binary_exists(command: &'static str) -> bool {
    which::which(command).is_ok()
}

fn env_var_is_set(env_var_name: &str) -> bool {
    std::env::var_os(env_var_name).is_some()
}

pub fn get_clipboard_content() -> Result<String> {
    CLIPBOARD_PROVIDER
        .get_or_init(|| get_clipboard_provider())
        .get_contents()
}

pub fn execute_copy_command(text: String) -> Result<()> {
    CLIPBOARD_PROVIDER
        .get_or_init(|| get_clipboard_provider())
        .set_contents(text)
}
