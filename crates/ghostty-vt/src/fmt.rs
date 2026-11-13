//! Format terminal content as plain text, VT sequences, or HTML.
//!
//! A formatter captures a reference to a terminal and formatting options.
//! It can be used repeatedly to produce output that reflects the current
//! terminal state at the time of each format call.
//!
//! Adapted from libghostty-vt 0.2.2 (MIT OR Apache-2.0).
use std::{io::Write, marker::PhantomData, ptr::NonNull};

use crate::{
    alloc::{Bytes, Object},
    error::{Error, Result, from_result},
    ffi,
    selection::Selection,
    terminal::Terminal,
};

/// Formatter that formats terminal content.
#[derive(Debug)]
pub struct Formatter<'t> {
    inner: Object<ffi::FormatterImpl>,
    _terminal: PhantomData<&'t Terminal>,
}

/// Options for [creating a terminal formatter](Formatter::new).
///
/// Every option starts off: plain text, no unwrapping, no trimming, no
/// extras, and the whole screen.
#[derive(Debug)]
pub struct FormatterOptions<'t, 's> {
    format: Format,
    unwrap: bool,
    trim: bool,
    extras: ffi::FormatterExtras,
    selection: Option<ffi::Selection>,
    _phan: PhantomData<&'s Selection<'t>>,
}

// The `GHOSTTY_FORMATTER_EXTRA_*` bits of `GhosttyFormatterExtras`.
const EXTRA_PALETTE: u32 = 1 << 0;
const EXTRA_MODES: u32 = 1 << 1;
const EXTRA_SCROLLING_REGION: u32 = 1 << 2;
const EXTRA_TABSTOPS: u32 = 1 << 3;
const EXTRA_PWD: u32 = 1 << 4;
const EXTRA_KEYBOARD: u32 = 1 << 5;
const EXTRA_CURSOR: u32 = 1 << 6;
const EXTRA_STYLE: u32 = 1 << 7;
const EXTRA_HYPERLINK: u32 = 1 << 8;
const EXTRA_PROTECTION: u32 = 1 << 9;
const EXTRA_KITTY_KEYBOARD: u32 = 1 << 10;
const EXTRA_CHARSETS: u32 = 1 << 11;

impl<'t, 's> FormatterOptions<'t, 's> {
    /// Create a new set of options for [creating a terminal formatter](Formatter::new).
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: Format::Plain,
            unwrap: false,
            trim: false,
            extras: 0,
            selection: None,
            _phan: PhantomData,
        }
    }
    /// Specify the output format to emit.
    #[must_use]
    pub fn with_format(mut self, value: Format) -> Self {
        self.format = value;
        self
    }
    /// Specify whether to unwrap soft-wrapped lines.
    #[must_use]
    pub fn with_unwrap(mut self, value: bool) -> Self {
        self.unwrap = value;
        self
    }
    /// Specify whether to trim trailing whitespace on non-blank lines.
    #[must_use]
    pub fn with_trim(mut self, value: bool) -> Self {
        self.trim = value;
        self
    }
    /// Specify the selection to restrict output to a range.
    ///
    /// If a selection is not given, the formatter defaults to formatting
    /// the entire screen.
    #[must_use]
    pub fn with_selection(mut self, value: &'s Selection<'t>) -> Self {
        self.selection = Some(value.inner);
        self
    }

    /// Restrict output to a raw selection of the same terminal.
    pub(crate) fn with_raw_selection(mut self, value: ffi::Selection) -> Self {
        self.selection = Some(value);
        self
    }

    fn extra(mut self, bit: u32, value: bool) -> Self {
        if value {
            self.extras |= bit;
        } else {
            self.extras &= !bit;
        }
        self
    }

    // --- Extra settings --- //

    /// Specify whether to emit the palette using OSC 4 sequences.
    #[must_use]
    pub fn with_palette(self, value: bool) -> Self {
        self.extra(EXTRA_PALETTE, value)
    }
    /// Specify terminal modes that differ from their defaults using CSI h/l.
    #[must_use]
    pub fn with_modes(self, value: bool) -> Self {
        self.extra(EXTRA_MODES, value)
    }
    /// Specify whether to emit scrolling region state using DECSTBM and DECSLRM sequences.
    #[must_use]
    pub fn with_scrolling_region(self, value: bool) -> Self {
        self.extra(EXTRA_SCROLLING_REGION, value)
    }
    /// Specify tabstop positions by clearing all tabs and setting each one.
    #[must_use]
    pub fn with_tabstops(self, value: bool) -> Self {
        self.extra(EXTRA_TABSTOPS, value)
    }
    /// Specify the present working directory using OSC 7.
    #[must_use]
    pub fn with_pwd(self, value: bool) -> Self {
        self.extra(EXTRA_PWD, value)
    }
    /// Specify keyboard modes such as ModifyOtherKeys.
    #[must_use]
    pub fn with_keyboard(self, value: bool) -> Self {
        self.extra(EXTRA_KEYBOARD, value)
    }

    // --- Screen settings --- //

    /// Specify whether to emit cursor position using CUP (CSI H).
    #[must_use]
    pub fn with_cursor(self, value: bool) -> Self {
        self.extra(EXTRA_CURSOR, value)
    }
    /// Emit current SGR style state based on the cursor's active style_id.
    #[must_use]
    pub fn with_style(self, value: bool) -> Self {
        self.extra(EXTRA_STYLE, value)
    }
    /// Emit current hyperlink state using OSC 8 sequences.
    #[must_use]
    pub fn with_hyperlink(self, value: bool) -> Self {
        self.extra(EXTRA_HYPERLINK, value)
    }
    /// Emit character protection mode using DECSCA.
    #[must_use]
    pub fn with_protection(self, value: bool) -> Self {
        self.extra(EXTRA_PROTECTION, value)
    }
    /// Emit Kitty keyboard protocol state using CSI > u and CSI = sequences.
    #[must_use]
    pub fn with_kitty_keyboard(self, value: bool) -> Self {
        self.extra(EXTRA_KITTY_KEYBOARD, value)
    }
    /// Emit character set designations and invocations.
    #[must_use]
    pub fn with_charsets(self, value: bool) -> Self {
        self.extra(EXTRA_CHARSETS, value)
    }
}

impl Default for FormatterOptions<'_, '_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'t> Formatter<'t> {
    /// Create a formatter for a terminal's active screen.
    pub fn new(terminal: &'t Terminal, opts: FormatterOptions<'t, '_>) -> Result<Self> {
        let mut raw: ffi::Formatter = std::ptr::null_mut();

        let result = unsafe {
            ffi::ghostty_formatter_terminal_new(std::ptr::null(), &raw mut raw, terminal.as_raw())
        };
        from_result(result)?;
        let formatter = Self {
            inner: Object::new(raw)?,
            _terminal: PhantomData,
        };

        let emit: ffi::FormatterFormat::Type = opts.format.into();
        formatter.set(ffi::FormatterOption::EMIT, &raw const emit)?;
        formatter.set(ffi::FormatterOption::UNWRAP, &raw const opts.unwrap)?;
        formatter.set(ffi::FormatterOption::TRIM, &raw const opts.trim)?;
        formatter.set(ffi::FormatterOption::EXTRAS, &raw const opts.extras)?;
        if let Some(selection) = &opts.selection {
            formatter.set(
                ffi::FormatterOption::SELECTION,
                std::ptr::from_ref(selection),
            )?;
        }
        Ok(formatter)
    }

    fn set<T>(&self, option: ffi::FormatterOption::Type, value: *const T) -> Result<()> {
        from_result(unsafe {
            ffi::ghostty_formatter_set(self.inner.as_raw(), option, value.cast())
        })
    }

    /// Run the formatter and return an allocated buffer with the output.
    ///
    /// Each call formats the current terminal state.
    pub fn format_alloc(&mut self) -> Result<Bytes> {
        let mut bytes = std::ptr::null_mut();
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_alloc(
                self.inner.as_raw(),
                std::ptr::null(),
                std::ptr::from_mut(&mut bytes),
                std::ptr::from_mut(&mut len),
            )
        };
        from_result(result)?;

        // Empty output comes back as a null pointer with a length of zero.
        let ptr = NonNull::new(bytes).unwrap_or(NonNull::dangling());
        Ok(unsafe { Bytes::from_raw_parts(ptr, len, std::ptr::null()) })
    }

    /// Run the formatter and stream the output to a writer.
    ///
    /// Each call formats the current terminal state. A write error stops the
    /// format and returns [`Error::IoError`].
    pub fn format_to(&mut self, writer: &mut dyn Write) -> Result<()> {
        crate::io::with_writer(writer, |w| unsafe {
            ffi::ghostty_formatter_format(self.inner.as_raw(), w)
        })
    }

    /// Run the formatter and produce output into the caller-provided buffer.
    ///
    /// Each call formats the current terminal state. If the buffer is too small,
    /// returns `Err(Error::OutOfSpace { required })` where `required` is the
    /// required size. The caller can then retry with a larger buffer.
    pub fn format_buf(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_buf(
                self.inner.as_raw(),
                std::ptr::from_mut(buf).cast(),
                buf.len(),
                std::ptr::from_mut(&mut len),
            )
        };
        crate::error::from_result_with_len(result, len)
    }

    /// Query the required buffer size for the formatted output.
    ///
    /// The result can be used to create a sufficiently large buffer
    /// for [`Formatter::format_buf`].
    pub fn format_len(&mut self) -> Result<usize> {
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_buf(
                self.inner.as_raw(),
                std::ptr::null_mut(),
                0,
                std::ptr::from_mut(&mut len),
            )
        };
        // Output fails with OutOfSpace; empty output needs no space and
        // succeeds with a length of zero.
        match from_result(result) {
            Err(Error::OutOfSpace { .. }) | Ok(()) => Ok(len),
            Err(e) => Err(e),
        }
    }
}

impl Drop for Formatter<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_formatter_free(self.inner.as_raw()) }
    }
}

/// Output format.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
pub enum Format {
    /// Plain text (no escape sequences).
    Plain = ffi::FormatterFormat::PLAIN,
    /// VT sequences preserving colors, styles, URLs, etc.
    Vt = ffi::FormatterFormat::VT,
    /// HTML with inline styles.
    Html = ffi::FormatterFormat::HTML,
}
