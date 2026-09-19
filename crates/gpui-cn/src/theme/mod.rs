//! The theme: a small config per appearance, the tokens derived from it,
//! and the preferences that shape motion and cursors.

mod color;
mod config;
pub mod fonts;
mod motion;
mod tokens;

pub use color::{hex, mix, to_hex, try_hex};
pub use config::{SemanticColors, ThemeConfig, ThemeFonts};
pub use motion::MotionTokens;
pub use tokens::{MetricTokens, ThemeTokens};

use gpui_kit::{
    App, BorrowAppContext as _, Global, Pixels, Window, WindowAppearance,
    base::{self, ThemeAppearance},
    px,
};

use tokens::Metrics;

/// Which appearance the theme follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    /// Follow the operating system.
    #[default]
    System,
    /// Always the light config.
    Light,
    /// Always the dark config.
    Dark,
}

/// Whether motion is reduced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReduceMotion {
    /// Follow the operating system's accessibility setting.
    #[default]
    System,
    /// Reduce motion regardless of the system.
    On,
    /// Animate regardless of the system.
    Off,
}

/// The gpui-cn theme, a GPUI global.
///
/// Public fields are the inputs. Edit them through [`Theme::update`], which
/// re-derives the tokens, projects them onto `gpui_base::Theme`, applies the
/// motion preference, and refreshes every window. Components read the
/// derived [`ThemeTokens`] through [`ActiveTheme::theme`].
///
/// gpui-cn owns `gpui_base::Theme` the same way `gpui-component` does:
/// every change is projected onto it, and nothing reads it back.
#[non_exhaustive]
pub struct Theme {
    /// Which config is active.
    pub mode: ThemeMode,
    /// The config used in light appearance.
    pub light: ThemeConfig,
    /// The config used in dark appearance.
    pub dark: ThemeConfig,
    /// The base corner radius; the radius scale derives from it.
    pub radius: Pixels,
    /// The interface font size. It is also the window rem size, so it is
    /// the zoom control for everything on the rem scale.
    pub ui_font_size: Pixels,
    /// The code font size.
    pub code_font_size: Pixels,
    /// Whether motion is reduced.
    pub reduce_motion: ReduceMotion,
    /// Durations and curves components animate with.
    pub motion: MotionTokens,
    /// Pointer cursor on interactive controls. Off keeps the platform arrow
    /// on buttons; links always use the pointer.
    pub pointer_cursors: bool,
    /// Whether elevated surfaces cast shadows.
    pub shadow: bool,

    system_appearance: ThemeAppearance,
    tokens: ThemeTokens,
    /// What `App::reduce_motion` held before gpui-cn first overrode it, so
    /// `ReduceMotion::System` can hand the flag back to base.
    reduce_motion_before_override: Option<bool>,
}

impl Global for Theme {}

impl Theme {
    /// Installs the theme and projects it onto `gpui_base::Theme`.
    pub(crate) fn init(cx: &mut App) {
        let mut theme = Self::new(appearance_of(cx.window_appearance()));
        theme.resolve(cx);
        cx.set_global(theme);
    }

    fn new(system_appearance: ThemeAppearance) -> Self {
        let light = ThemeConfig::light();
        let metrics = Metrics {
            radius: px(10.),
            ui_font_size: px(16.),
            code_font_size: px(13.),
        };
        Self {
            mode: ThemeMode::System,
            tokens: ThemeTokens::derive(&light, ThemeAppearance::Light, metrics),
            light,
            dark: ThemeConfig::dark(),
            radius: metrics.radius,
            ui_font_size: metrics.ui_font_size,
            code_font_size: metrics.code_font_size,
            reduce_motion: ReduceMotion::System,
            motion: MotionTokens::default(),
            pointer_cursors: false,
            shadow: true,
            system_appearance,
            reduce_motion_before_override: None,
        }
    }

    /// The theme global. Panics before [`crate::init`].
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// The derived tokens for the active appearance.
    pub fn tokens(&self) -> &ThemeTokens {
        &self.tokens
    }

    /// The active appearance after resolving `mode`.
    pub fn appearance(&self) -> ThemeAppearance {
        match self.mode {
            ThemeMode::System => self.system_appearance,
            ThemeMode::Light => ThemeAppearance::Light,
            ThemeMode::Dark => ThemeAppearance::Dark,
        }
    }

    /// Whether the active appearance is dark.
    pub fn is_dark(&self) -> bool {
        self.appearance() == ThemeAppearance::Dark
    }

    /// The config for the active appearance.
    pub fn active_config(&self) -> &ThemeConfig {
        if self.is_dark() {
            &self.dark
        } else {
            &self.light
        }
    }

    /// Edits the theme, then re-derives tokens, projects them onto base,
    /// applies the motion preference, and refreshes every window.
    pub fn update<R>(cx: &mut App, edit: impl FnOnce(&mut Theme) -> R) -> R {
        let result = cx.update_global::<Self, _>(|theme, cx| {
            let result = edit(theme);
            theme.resolve(cx);
            result
        });
        cx.refresh_windows();
        result
    }

    /// Switches the mode.
    pub fn change(mode: ThemeMode, cx: &mut App) {
        Self::update(cx, |theme| theme.mode = mode);
    }

    /// Reads the system appearance again. [`crate::Root`] calls this when the
    /// window appearance changes; it matters only in [`ThemeMode::System`].
    pub fn sync_system_appearance(window: Option<&Window>, cx: &mut App) {
        let appearance = window
            .map(|window| window.appearance())
            .unwrap_or_else(|| cx.window_appearance());
        let appearance = appearance_of(appearance);
        if Self::global(cx).system_appearance == appearance {
            return;
        }
        Self::update(cx, |theme| theme.system_appearance = appearance);
    }

    /// Sets both font sizes at once, keeping their ratio.
    pub fn set_ui_font_size(cx: &mut App, size: Pixels) {
        Self::update(cx, |theme| {
            let ratio = f32::from(theme.code_font_size) / f32::from(theme.ui_font_size);
            theme.ui_font_size = size;
            theme.code_font_size = size * ratio;
        });
    }

    /// Re-derives tokens, projects them onto base, and applies the motion
    /// preference.
    fn resolve(&mut self, cx: &mut App) {
        let metrics = Metrics {
            radius: self.radius,
            ui_font_size: self.ui_font_size,
            code_font_size: self.code_font_size,
        };
        self.tokens = ThemeTokens::derive(self.active_config(), self.appearance(), metrics);
        self.sync_base(cx);
        self.apply_reduce_motion(cx);
    }

    /// Writes the derived tokens into `gpui_base::Theme`, keeping base's own
    /// scrollbar and resize-handle styles.
    fn sync_base(&self, cx: &mut App) {
        let base_theme = base::Theme::global_mut(cx);
        base_theme.appearance = self.tokens.appearance;
        base_theme.tokens = self.tokens.base.clone();
        base_theme.resizable = base::ResizableTheme {
            handle: Some(self.tokens.border()),
            active_handle: Some(self.tokens.ring()),
        };
    }

    fn apply_reduce_motion(&mut self, cx: &mut App) {
        match self.reduce_motion {
            ReduceMotion::System => {
                if let Some(previous) = self.reduce_motion_before_override.take() {
                    cx.set_reduce_motion(previous);
                    base::apply_system_reduce_motion(cx);
                }
            }
            ReduceMotion::On | ReduceMotion::Off => {
                if self.reduce_motion_before_override.is_none() {
                    self.reduce_motion_before_override = Some(cx.reduce_motion());
                }
                cx.set_reduce_motion(self.reduce_motion == ReduceMotion::On);
            }
        }
    }
}

/// Derived tokens for unit tests in other modules.
#[cfg(test)]
pub(crate) fn test_tokens(config: &ThemeConfig, appearance: ThemeAppearance) -> ThemeTokens {
    ThemeTokens::derive(
        config,
        appearance,
        Metrics {
            radius: px(10.),
            ui_font_size: px(16.),
            code_font_size: px(13.),
        },
    )
}

fn appearance_of(appearance: WindowAppearance) -> ThemeAppearance {
    match appearance {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeAppearance::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeAppearance::Light,
    }
}

/// Access to the derived theme tokens from a context.
pub trait ActiveTheme {
    /// The tokens for the active appearance.
    fn theme(&self) -> &ThemeTokens;
}

impl ActiveTheme for App {
    #[inline(always)]
    fn theme(&self) -> &ThemeTokens {
        Theme::global(self).tokens()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn init_owns_base_and_projects_tokens(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
            let theme = Theme::global(cx);
            assert_eq!(theme.mode, ThemeMode::System);
            let base_theme = base::Theme::global(cx);
            assert_eq!(base_theme.tokens.colors, theme.tokens().base.colors);
            assert_eq!(base_theme.appearance, theme.appearance());
            assert_eq!(base_theme.tokens.radius.lg, px(10.));
        });
    }

    #[gpui_kit::test]
    fn change_switches_tokens_and_base(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
            Theme::change(ThemeMode::Dark, cx);
            assert!(cx.theme().is_dark());
            assert_eq!(cx.theme().background(), hex("#181818"));
            assert_eq!(base::Theme::global(cx).appearance, ThemeAppearance::Dark);
            Theme::change(ThemeMode::Light, cx);
            assert_eq!(cx.theme().background(), hex("#ffffff"));
            assert_eq!(base::Theme::global(cx).appearance, ThemeAppearance::Light);
        });
    }

    #[gpui_kit::test]
    fn update_re_derives_from_edited_inputs(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
            Theme::update(cx, |theme| {
                theme.mode = ThemeMode::Light;
                theme.light.accent = hex("#ff0000");
                theme.radius = px(5.);
            });
            assert_eq!(cx.theme().ring(), hex("#ff0000"));
            assert_eq!(cx.theme().radius_md(), px(4.));
            assert_eq!(base::Theme::global(cx).tokens.colors.ring, hex("#ff0000"));
        });
    }

    #[gpui_kit::test]
    fn font_size_scales_typography_together(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
            Theme::set_ui_font_size(cx, px(20.));
            let theme = Theme::global(cx);
            assert_eq!(theme.ui_font_size, px(20.));
            assert_eq!(theme.code_font_size, px(16.25));
            assert_eq!(cx.theme().base.typography.md.size, px(20.));
            assert_eq!(base::Theme::global(cx).tokens.typography.md.size, px(20.));
        });
    }

    #[gpui_kit::test]
    fn reduce_motion_preference_overrides_and_hands_back(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
            assert!(!cx.reduce_motion());
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
            assert!(cx.reduce_motion());
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::Off);
            assert!(!cx.reduce_motion());
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
            assert!(cx.reduce_motion());
            Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::System);
            // Under the test scheduler base does not consult the platform, so
            // the flag returns to what it held before the first override.
            assert!(!cx.reduce_motion());
        });
    }
}
