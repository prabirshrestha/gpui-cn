//! A terminal on libghostty-vt, the terminal emulation library from
//! [Ghostty](https://ghostty.org), behind the `ghostty` feature.
//!
//! The pieces:
//!
//! - [`TerminalState`] is the entity: it holds the latest [`TerminalFrame`],
//!   the program's [`TerminalStatus`], the selection and the focus, and it
//!   sends [`TerminalInput`] to its source.
//! - [`Terminal`] is the element that paints a state and routes keys, the
//!   pointer and the input method to it.
//! - A [`FrameSource`] produces frames. [`Engine`] runs a [`ByteSource`]
//!   through a Ghostty terminal on an owner thread; [`LocalPty`] (feature
//!   `ghostty-pty`) is the byte source for a local program; [`StreamSource`]
//!   is one the application feeds itself, such as a remote session or its
//!   own pty; [`FixtureSource`] parses canned bytes for tests and galleries.
//! - [`shell`] resolves the user's shell, its login command and the
//!   environment the way Ghostty does, for applications that start the
//!   program themselves.
//!
//! [`crate::init`] registers the default key bindings in the [`KEY_CONTEXT`]
//! key context.
//!
//! The grid uses the theme's code font and code font size unless the
//! [`TerminalAppearance`] names others, so the `jetbrains-mono` feature
//! gives it JetBrains Mono. The colors are Ghostty's defaults;
//! [`TerminalColors::from_ghostty_theme`] reads a Ghostty theme file.
//!
//! The engine never blocks the UI: frames arrive through a latest-wins slot
//! and an async wake, input goes through a bounded queue that Ctrl+C jumps,
//! a hidden terminal keeps parsing but publishes nothing, and synchronized
//! output (mode 2026) holds frames for at most one second.
//!
//! Adapted from tt v2 (Apache-2.0), crates/desktop/src/terminal, itself
//! derived from Herdr; block, decoration and element structure adapted from
//! Muxy (MIT).

mod appearance;
mod block;
mod colors;
mod core;
mod cursor;
mod decoration;
mod element;
mod engine;
pub mod frame;
mod geometry;
mod ime;
pub mod input;
mod keys;
mod links;
mod options;
#[cfg(all(
    feature = "ghostty-pty",
    not(any(target_os = "ios", target_os = "android"))
))]
mod pty;
mod selection;
pub mod shell;
mod source;
mod state;
mod stream;
mod theme;

pub use appearance::TerminalAppearance;
pub use element::{KEY_CONTEXT, Terminal};
pub use engine::{
    ByteHandle, ByteSink, ByteSource, Engine, ExitStatus, InputRejected, TerminalSnapshot,
    TerminalStatus,
};
pub use frame::{TerminalColors, TerminalFrame, Viewport};
pub use input::TerminalInput;
pub use options::{EngineOptions, LocalTerminalOptions, ShellIntegration, WorkingDirectory};
#[cfg(all(
    feature = "ghostty-pty",
    not(any(target_os = "ios", target_os = "android"))
))]
pub use pty::LocalPty;
pub use source::{FixtureSource, FrameHandle, FrameSink, FrameSource, StartOptions};
pub use state::{TerminalConfig, TerminalEvent, TerminalState};
pub use stream::{StreamEvent, StreamPeer, StreamSource};
pub use theme::ThemeError;

/// Actions dispatched in the [`KEY_CONTEXT`] key context.
pub mod actions {
    gpui_kit::actions!(
        terminal,
        [
            /// Copy the selection.
            Copy,
            /// Paste the clipboard.
            Paste,
            /// Select the viewport.
            SelectAll,
            /// Drop the selection; propagates when there is none.
            ClearSelection,
            /// Clear the screen through the shell.
            Clear,
            /// Scroll to the top of scrollback.
            ScrollToTop,
            /// Scroll to the bottom.
            ScrollToBottom,
            /// Scroll one page toward older output.
            ScrollPageUp,
            /// Scroll one page toward newer output.
            ScrollPageDown,
            /// Scroll one line toward older output.
            ScrollLineUp,
            /// Scroll one line toward newer output.
            ScrollLineDown,
            /// Send Tab to the program.
            SendTab,
            /// Send Shift-Tab to the program.
            SendBackTab,
        ]
    );
}

/// Registers the default key bindings. [`crate::init`] calls it.
pub(crate) fn init(cx: &mut gpui_kit::App) {
    use actions::{
        Clear, ClearSelection, Copy, Paste, ScrollLineDown, ScrollLineUp, ScrollPageDown,
        ScrollPageUp, ScrollToBottom, ScrollToTop, SelectAll, SendBackTab, SendTab,
    };
    use gpui_kit::KeyBinding;
    let context = Some(KEY_CONTEXT);
    let (copy, paste, select_all, clear) = if cfg!(target_os = "macos") {
        ("cmd-c", "cmd-v", "cmd-a", "cmd-k")
    } else {
        (
            "ctrl-shift-c",
            "ctrl-shift-v",
            "ctrl-shift-a",
            "ctrl-shift-k",
        )
    };
    cx.bind_keys([
        KeyBinding::new("tab", SendTab, context),
        KeyBinding::new("shift-tab", SendBackTab, context),
        KeyBinding::new(copy, Copy, context),
        KeyBinding::new(paste, Paste, context),
        KeyBinding::new(select_all, SelectAll, context),
        KeyBinding::new(clear, Clear, context),
        KeyBinding::new("escape", ClearSelection, context),
        KeyBinding::new("shift-pageup", ScrollPageUp, context),
        KeyBinding::new("shift-pagedown", ScrollPageDown, context),
        KeyBinding::new("shift-home", ScrollToTop, context),
        KeyBinding::new("shift-end", ScrollToBottom, context),
        KeyBinding::new("shift-up", ScrollLineUp, context),
        KeyBinding::new("shift-down", ScrollLineDown, context),
    ]);
}
