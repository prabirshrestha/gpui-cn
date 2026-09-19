//! shadcn-style components for GPUI, built on `gpui-base`.
//!
//! gpui-cn depends on `gpui-kit` with the component layer off, so it adds
//! GPUI and `gpui-base` and nothing else.
//!
//! Initialize `gpui-base` first (through `gpui_kit::init`), then call
//! [`init`] once before opening windows, and put [`Root`] at the first
//! level of each window that gpui-cn should own:
//!
//! ```no_run
//! use gpui_kit::*;
//!
//! struct Hello;
//!
//! impl Render for Hello {
//!     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
//!         div().p_4().child(
//!             gpui_cn::Button::new("save")
//!                 .primary()
//!                 .label("Save")
//!                 .on_click(|_, window, cx| {
//!                     let _ = window.prompt(PromptLevel::Info, "Saved", None, &["OK"], cx);
//!                 }),
//!         )
//!     }
//! }
//!
//! fn main() {
//!     gpui_kit::application().run(|cx| {
//!         gpui_kit::init(cx);
//!         gpui_cn::init(cx);
//!         cx.spawn(async move |cx| {
//!             cx.open_window(WindowOptions::default(), |window, cx| {
//!                 let view = cx.new(|_| Hello);
//!                 cx.new(|cx| gpui_cn::Root::new(view, window, cx))
//!             })
//!             .expect("failed to open window");
//!         })
//!         .detach();
//!     });
//! }
//! ```

mod button;
mod icon;
mod root;
pub mod theme;
mod tooltip;
mod tooltip_host;

pub use button::{Button, ButtonSize, ButtonVariant};
pub use gpui_kit;
pub use icon::{Icon, IconSource};
pub use root::Root;
pub use theme::{ActiveTheme, ReduceMotion, Theme, ThemeConfig, ThemeMode, ThemeTokens};
pub use tooltip::{Tooltip, TooltipExt, TooltipTrigger};
pub use tooltip_host::TooltipHost;

use gpui_kit::{App, Global};

/// Everything an application normally imports from gpui-cn.
pub mod prelude {
    pub use crate::{
        ActiveTheme, Button, ButtonSize, ButtonVariant, Icon, ReduceMotion, Root, Theme, ThemeMode,
        TooltipExt,
    };
    pub use gpui_kit::base::{Disableable, Placement, Selectable, StyledExt};
    pub use gpui_kit::prelude::FluentBuilder;
}

/// Marks that [`init`] ran, so a second call is a no-op.
struct Initialized;

impl Global for Initialized {}

/// Initializes gpui-cn. Call it once, after `gpui_base::init` (normally
/// through `gpui_kit::init`) and before opening windows.
///
/// Installs the built-in light and dark themes and projects them onto
/// `gpui_base::Theme`, as `gpui_component::init` does for its own theme.
/// Calling it twice changes nothing. In an application that also runs
/// `gpui_component::init`, the later of the two owns the base theme and
/// the other layer follows its colors.
///
/// # Panics
///
/// If `gpui_base::init` has not run. gpui-cn never initializes base itself:
/// the host owns that, as it does for every other layer on base.
pub fn init(cx: &mut App) {
    if cx.has_global::<Initialized>() {
        return;
    }
    assert!(
        base_initialized(cx),
        "gpui_cn::init needs gpui_base::init first; call gpui_kit::init(cx) before it"
    );
    Theme::init(cx);
    root::init(cx);
    cx.set_global(Initialized);
}

/// Whether `gpui_base::init` has run in this application.
///
/// `gpui_base::init` installs `GlobalState`, which nothing else creates, so
/// its presence is the signal.
fn base_initialized(cx: &App) -> bool {
    cx.has_global::<gpui_kit::base::GlobalState>()
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn init_is_idempotent_and_skips_base_when_already_initialized(cx: &mut TestAppContext) {
        cx.update(|cx| {
            assert!(!super::base_initialized(cx));
            gpui_kit::init(cx);
            assert!(super::base_initialized(cx));
            super::init(cx);
            super::init(cx);
            assert!(cx.has_global::<super::Initialized>());
        });
    }

    #[gpui_kit::test]
    #[should_panic(expected = "needs gpui_base::init first")]
    fn init_refuses_to_run_before_base(cx: &mut TestAppContext) {
        cx.update(super::init);
    }
}
