//! Where a terminal's frames come from.
//!
//! A [`FrameSource`] produces [`TerminalSnapshot`]s into a [`FrameSink`] and
//! accepts input through a [`FrameHandle`]. The engine in this crate is one
//! source; a fixture that parses canned bytes is another, and a host can
//! bring its own, for example frames received from a remote server.

use std::io;
use std::sync::{Arc, Mutex};

use gpui_kit::base::async_util;

use crate::terminal::core::{Core, Effects};
use crate::terminal::engine::{InputRejected, TerminalSnapshot};
use crate::terminal::frame::{CellSelection, CursorShape, TerminalColors, Viewport};
use crate::terminal::input::TerminalInput;
use crate::terminal::options::EngineOptions;

/// Where the engine publishes snapshots: a latest-wins slot plus a wake.
///
/// The UI awaits the receiver, takes the slot and applies it. Several
/// publishes between two wakes collapse into the latest snapshot.
#[derive(Clone)]
pub struct FrameSink {
    latest: Arc<Mutex<Option<Arc<TerminalSnapshot>>>>,
    wake: async_util::Sender<()>,
}

impl std::fmt::Debug for FrameSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameSink").finish_non_exhaustive()
    }
}

impl FrameSink {
    /// A sink and the receiver the UI awaits.
    #[must_use]
    pub fn new() -> (Self, async_util::Receiver<()>) {
        let (wake, rx) = async_util::unbounded();
        (
            Self {
                latest: Arc::new(Mutex::new(None)),
                wake,
            },
            rx,
        )
    }

    /// Publish a snapshot and wake the UI.
    pub fn publish(&self, snapshot: Arc<TerminalSnapshot>) {
        *self
            .latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(snapshot);
        let _ = self.wake.try_send(());
    }

    /// Take the latest snapshot, if one was published since the last take.
    pub fn take(&self) -> Option<Arc<TerminalSnapshot>> {
        self.latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

/// What a source needs to start.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct StartOptions {
    /// The initial grid.
    pub(crate) viewport: Viewport,
    /// The initial colors.
    pub(crate) colors: TerminalColors,
    /// Whether the terminal starts visible.
    pub(crate) visible: bool,
    /// The default cursor shape and blink, when the host overrides them.
    pub(crate) cursor: Option<(CursorShape, bool)>,
}

impl Default for StartOptions {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            colors: TerminalColors::default(),
            visible: true,
            cursor: None,
        }
    }
}

impl StartOptions {
    /// Set the initial grid.
    #[must_use]
    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }

    /// Set the initial colors.
    #[must_use]
    pub fn with_colors(mut self, colors: TerminalColors) -> Self {
        self.colors = colors;
        self
    }

    /// Set whether the terminal starts visible.
    #[must_use]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set the default cursor shape and blink.
    #[must_use]
    pub fn with_cursor(mut self, shape: CursorShape, blinking: bool) -> Self {
        self.cursor = Some((shape, blinking));
        self
    }

    /// The initial grid.
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// The initial colors.
    pub fn colors(&self) -> &TerminalColors {
        &self.colors
    }

    /// Whether the terminal starts visible.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// The default cursor shape and blink, when the host overrides them.
    pub fn cursor(&self) -> Option<(CursorShape, bool)> {
        self.cursor
    }
}

/// A producer of terminal snapshots.
pub trait FrameSource: 'static {
    /// Start publishing into `sink` and return the handle for input.
    fn start(
        self: Box<Self>,
        sink: FrameSink,
        options: StartOptions,
    ) -> io::Result<Box<dyn FrameHandle>>;
}

/// The UI's side of a running [`FrameSource`]. Nothing here may block.
pub trait FrameHandle: 'static {
    /// Queue input.
    fn input(&self, input: TerminalInput) -> Result<(), InputRejected>;
    /// Queued input bytes not yet delivered.
    fn input_backlog(&self) -> usize {
        0
    }
    /// The grid changed.
    fn resize(&self, viewport: Viewport);
    /// The terminal became visible or hidden.
    fn set_visible(&self, visible: bool);
    /// The last frame was painted; the source may publish the next one.
    fn request_frame(&self);
    /// The default colors changed.
    fn set_colors(&self, colors: TerminalColors) {
        let _ = colors;
    }
    /// The default cursor changed.
    fn set_cursor(&self, shape: CursorShape, blinking: bool) {
        let _ = (shape, blinking);
    }
    /// Ask for the selected text, which may reach scrollback the frame does
    /// not carry. `None` means the caller must read the frame instead.
    fn copy(&self, selection: CellSelection) -> Option<async_util::Receiver<io::Result<String>>> {
        let _ = selection;
        None
    }
    /// Stop the source. Idempotent.
    fn close(&self);
}

/// A source that parses canned bytes once, with no program behind it.
///
/// The frame is rebuilt at every resize. Input is recorded and can be read
/// back through [`FixtureSource::inputs`], which makes it the source for UI
/// tests and galleries.
#[derive(Debug)]
pub struct FixtureSource {
    bytes: Vec<u8>,
    options: EngineOptions,
    inputs: Arc<Mutex<Vec<TerminalInput>>>,
}

impl FixtureSource {
    /// A fixture rendering `bytes` as program output.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            options: EngineOptions::default(),
            inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set the engine options the fixture parses with.
    #[must_use]
    pub fn with_options(mut self, options: EngineOptions) -> Self {
        self.options = options;
        self
    }

    /// Everything sent to the fixture so far, oldest first.
    #[must_use]
    pub fn inputs(&self) -> Arc<Mutex<Vec<TerminalInput>>> {
        self.inputs.clone()
    }
}

struct FixtureHandle {
    bytes: Vec<u8>,
    options: EngineOptions,
    colors: Mutex<TerminalColors>,
    cursor: Mutex<Option<(CursorShape, bool)>>,
    inputs: Arc<Mutex<Vec<TerminalInput>>>,
    sink: FrameSink,
}

impl FixtureHandle {
    fn publish(&self, viewport: Viewport) -> io::Result<()> {
        let colors = self
            .colors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let cursor = *self
            .cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut core = Core::new(viewport, &self.options, colors)?;
        if let Some((shape, blinking)) = cursor {
            core.set_cursor(shape, blinking)?;
        }
        let mut effects = Effects::default();
        core.write(&self.bytes, &mut effects);
        let frame = core
            .frame(true)?
            .ok_or_else(|| io::Error::other("fixture frame missing"))?;
        let mut snapshot = TerminalSnapshot::for_frame(frame);
        core.title().clone_into(&mut snapshot.title);
        snapshot.cwd = core.cwd().cloned();
        snapshot.bell_count = effects.bells;
        snapshot.at_prompt = core.at_prompt();
        self.sink.publish(Arc::new(snapshot));
        Ok(())
    }
}

impl FrameSource for FixtureSource {
    fn start(
        self: Box<Self>,
        sink: FrameSink,
        options: StartOptions,
    ) -> io::Result<Box<dyn FrameHandle>> {
        let handle = FixtureHandle {
            bytes: self.bytes,
            options: self.options,
            colors: Mutex::new(options.colors),
            cursor: Mutex::new(options.cursor),
            inputs: self.inputs,
            sink,
        };
        handle.publish(options.viewport)?;
        Ok(Box::new(handle))
    }
}

impl FrameHandle for FixtureHandle {
    fn input(&self, input: TerminalInput) -> Result<(), InputRejected> {
        self.inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(input);
        Ok(())
    }

    fn resize(&self, viewport: Viewport) {
        let _ = self.publish(viewport);
    }

    fn set_visible(&self, _visible: bool) {}

    fn request_frame(&self) {}

    fn set_colors(&self, colors: TerminalColors) {
        *self
            .colors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = colors;
    }

    fn set_cursor(&self, shape: CursorShape, blinking: bool) {
        *self
            .cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((shape, blinking));
    }

    fn close(&self) {}
}
