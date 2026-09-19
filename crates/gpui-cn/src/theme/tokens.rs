use gpui_kit::{
    Hsla, Pixels, SharedString,
    base::{
        ColorTokens, RadiusTokens, SemanticThemeTokens, ShadowTokens, SpacingTokens,
        TextStyleToken, ThemeAppearance, TypographyTokens,
    },
    px,
};

use super::{ThemeConfig, color::mix};

/// The base font size the typography scale is authored at.
const BASE_FONT_SIZE: f32 = 16.;

/// Everything a component reads: base's semantic tokens plus the few
/// roles gpui-cn components need that base does not define. Derived from a
/// [`ThemeConfig`] through [`ThemeTokens::derive`]; never edited by hand
/// except through [`Theme::update`](super::Theme::update).
///
/// Roles are added here only when a component reads them. Semantic colors
/// (success, warning, info, skill) stay on the config until then.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ThemeTokens {
    /// Light or dark.
    pub appearance: ThemeAppearance,
    /// Colors, radius, spacing, typography, and shadow shared with base.
    pub base: SemanticThemeTokens,
    /// The persistent selected surface: a sidebar row, a segmented choice,
    /// a pressed toggle. Stronger than the hover surface (`accent`).
    pub selected: Hsla,
    /// Link text. The accent, lifted toward the ink on a dark surface so
    /// it reads as text; the focus ring (`ring`) keeps the accent itself.
    pub link: Hsla,
    /// The tooltip pill: a step above the window surface on dark, the ink
    /// on light.
    pub tooltip: Hsla,
    /// Text on a tooltip.
    pub tooltip_foreground: Hsla,
}

/// Sizes that do not come from the config but still shape the tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Metrics {
    pub radius: Pixels,
    pub ui_font_size: Pixels,
    pub code_font_size: Pixels,
}

impl ThemeTokens {
    /// Derives the full token set from a config.
    ///
    /// `contrast` is read as a factor around 50: at 50 the ratios match the
    /// shadcn neutral palette; the built-in 45 (light) and 60 (dark) sit
    /// close to it. A dark surface gets a slightly lifted card and popover because a
    /// hairline alone does not separate them from the window.
    pub(super) fn derive(
        config: &ThemeConfig,
        appearance: ThemeAppearance,
        metrics: Metrics,
    ) -> Self {
        let dark = appearance == ThemeAppearance::Dark;
        let c = f32::from(config.contrast) / 50.;
        let surface = config.surface;
        let ink = config.ink;
        let toward_ink = |amount: f32| mix(surface, ink, amount);

        // Every fill is one OKLab step from the surface toward the ink,
        // scaled by contrast. The constants reproduce the reference
        // application's sampled values at its own contrast settings (45
        // light, 60 dark); separators and control borders keep one
        // distance in both appearances. On a dark surface the text roles
        // step a little toward the surface as the reference does (its
        // default text is #dfdfdf on #181818), and the elevated surfaces
        // lift because a hairline alone does not separate them.
        let soft = toward_ink(0.044 * c);
        let selected = toward_ink(if dark { 0.0867 } else { 0.10 } * c);
        let border = toward_ink(0.07);
        let input = toward_ink(0.10);
        let foreground = if dark { mix(ink, surface, 0.12) } else { ink };
        let muted_foreground = mix(ink, surface, if dark { 0.412 } else { 0.38 });
        let card = if dark {
            toward_ink(0.0475 * c)
        } else {
            surface
        };
        // The reference paints links #91c2fa on dark, paler than its
        // #539af8 accent, and the accent itself on light.
        let link = if dark {
            mix(config.accent, ink, 0.35)
        } else {
            config.accent
        };
        // The reference tooltip is #1b1b1b with white text on the dark
        // surface: a step so small it reads as the surface itself.
        let (tooltip, tooltip_foreground) = if dark {
            (toward_ink(0.016), ink)
        } else {
            (ink, surface)
        };
        // The reference draws its primary button a step short of the ink,
        // with the label a step short of the surface (#dfdfdf on #2d2d2d).
        let primary = mix(ink, surface, 0.12);
        let primary_foreground = mix(surface, ink, 0.11);
        let destructive_foreground = readable_on(config.semantic.destructive, surface, ink);

        let colors = ColorTokens {
            background: surface,
            foreground,
            surface: card,
            surface_foreground: foreground,
            primary,
            primary_foreground,
            secondary: soft,
            secondary_foreground: foreground,
            muted: soft,
            muted_foreground,
            accent: soft,
            accent_foreground: foreground,
            destructive: config.semantic.destructive,
            destructive_foreground,
            border,
            input,
            ring: config.accent,
            selection: config.accent.alpha(0.3),
        };

        let typography = typography(config, metrics);
        let shadow = ShadowTokens::elevations(if dark {
            gpui_kit::black().alpha(0.4)
        } else {
            ink.alpha(0.1)
        });

        Self {
            appearance,
            base: SemanticThemeTokens {
                colors,
                radius: radius_scale(metrics.radius),
                spacing: SpacingTokens::default(),
                typography,
                shadow,
            },
            selected,
            link,
            tooltip,
            tooltip_foreground,
        }
    }

    /// Whether the tokens are the dark set.
    pub fn is_dark(&self) -> bool {
        self.appearance == ThemeAppearance::Dark
    }

    /// The color tokens shared with base.
    pub fn colors(&self) -> &ColorTokens {
        &self.base.colors
    }

    /// The window background.
    pub fn background(&self) -> Hsla {
        self.base.colors.background
    }

    /// Default text.
    pub fn foreground(&self) -> Hsla {
        self.base.colors.foreground
    }

    /// High-emphasis actions: the default button.
    pub fn primary(&self) -> Hsla {
        self.base.colors.primary
    }

    /// Text on a primary surface.
    pub fn primary_foreground(&self) -> Hsla {
        self.base.colors.primary_foreground
    }

    /// Lower-emphasis filled actions and supporting surfaces.
    pub fn secondary(&self) -> Hsla {
        self.base.colors.secondary
    }

    /// Text on a secondary surface.
    pub fn secondary_foreground(&self) -> Hsla {
        self.base.colors.secondary_foreground
    }

    /// Subtle surfaces.
    pub fn muted(&self) -> Hsla {
        self.base.colors.muted
    }

    /// Descriptions, placeholders, helper text.
    pub fn muted_foreground(&self) -> Hsla {
        self.base.colors.muted_foreground
    }

    /// Hover, focus, and active surfaces.
    pub fn accent(&self) -> Hsla {
        self.base.colors.accent
    }

    /// Text on an accent surface.
    pub fn accent_foreground(&self) -> Hsla {
        self.base.colors.accent_foreground
    }

    /// Destructive actions and error emphasis.
    pub fn destructive(&self) -> Hsla {
        self.base.colors.destructive
    }

    /// Text on a destructive surface.
    pub fn destructive_foreground(&self) -> Hsla {
        self.base.colors.destructive_foreground
    }

    /// Default borders and separators.
    pub fn border(&self) -> Hsla {
        self.base.colors.border
    }

    /// Form control borders.
    pub fn input(&self) -> Hsla {
        self.base.colors.input
    }

    /// Focus rings.
    pub fn ring(&self) -> Hsla {
        self.base.colors.ring
    }

    /// Selected text background.
    pub fn selection(&self) -> Hsla {
        self.base.colors.selection
    }

    /// Interface font family.
    pub fn font_family(&self) -> &SharedString {
        &self.base.typography.sans
    }

    /// Code font family.
    pub fn mono_font_family(&self) -> &SharedString {
        &self.base.typography.mono
    }

    /// Radius for small controls: badges, checkboxes, menu items.
    pub fn radius_sm(&self) -> Pixels {
        self.base.radius.sm
    }

    /// Radius for buttons, inputs, and menu rows.
    pub fn radius_md(&self) -> Pixels {
        self.base.radius.md
    }

    /// The base radius: cards, popovers, dialogs.
    pub fn radius_lg(&self) -> Pixels {
        self.base.radius.lg
    }

    /// Radius for large surfaces: sheets and window panels.
    pub fn radius_xl(&self) -> Pixels {
        self.base.radius.xl
    }

    /// As round as the shape allows: circles and pills.
    pub fn radius_full(&self) -> Pixels {
        self.base.radius.full
    }
}

/// The shadcn radius scale from one base value.
fn radius_scale(radius: Pixels) -> RadiusTokens {
    RadiusTokens {
        none: px(0.),
        sm: radius * 0.6,
        md: radius * 0.8,
        lg: radius,
        xl: radius * 1.4,
        full: px(9999.),
    }
}

/// Base's typography scale, moved to the configured sizes and families.
fn typography(config: &ThemeConfig, metrics: Metrics) -> TypographyTokens {
    let defaults = TypographyTokens::default();
    let factor = f32::from(metrics.ui_font_size) / BASE_FONT_SIZE;
    let scale = |token: TextStyleToken| TextStyleToken {
        size: token.size * factor,
        line_height: token.line_height * factor,
        weight: token.weight,
    };
    let code_factor = f32::from(metrics.code_font_size) / f32::from(defaults.mono_md.size);
    TypographyTokens {
        sans: config.fonts.ui.clone().unwrap_or(defaults.sans),
        mono: config.fonts.code.clone().unwrap_or(defaults.mono),
        xs: scale(defaults.xs),
        sm: scale(defaults.sm),
        md: scale(defaults.md),
        lg: scale(defaults.lg),
        xl: scale(defaults.xl),
        mono_md: TextStyleToken {
            size: metrics.code_font_size,
            line_height: defaults.mono_md.line_height * code_factor,
            weight: defaults.mono_md.weight,
        },
    }
}

/// Picks `surface` or `ink` as text on a colored `background`.
///
/// Light text on a saturated mid-tone (the accent blue, a shadcn red) is the
/// convention even where a contrast formula would pick dark text, so the
/// lighter of the two is used unless the background is itself light.
fn readable_on(background: Hsla, surface: Hsla, ink: Hsla) -> Hsla {
    use super::color::lightness;
    const LIGHT_BACKGROUND: f32 = 0.72;
    let (lighter, darker) = if lightness(surface) >= lightness(ink) {
        (surface, ink)
    } else {
        (ink, surface)
    };
    if lightness(background) > LIGHT_BACKGROUND {
        darker
    } else {
        lighter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::{hex, lightness, to_hex};

    fn metrics() -> Metrics {
        Metrics {
            radius: px(10.),
            ui_font_size: px(16.),
            code_font_size: px(13.),
        }
    }

    fn light() -> ThemeTokens {
        ThemeTokens::derive(&ThemeConfig::light(), ThemeAppearance::Light, metrics())
    }

    fn dark() -> ThemeTokens {
        ThemeTokens::derive(&ThemeConfig::dark(), ThemeAppearance::Dark, metrics())
    }

    #[test]
    fn surfaces_step_away_from_the_background_in_both_appearances() {
        let light = light();
        let bg = lightness(light.background());
        assert!(lightness(light.secondary()) < bg);
        assert!(lightness(light.border()) < lightness(light.secondary()));
        assert!(lightness(light.input()) < lightness(light.border()));
        assert!(lightness(light.selected) < lightness(light.secondary()));
        assert_eq!(
            light.base.colors.surface,
            light.background(),
            "light cards sit flat"
        );
        assert!(lightness(light.muted_foreground()) > lightness(light.foreground()));

        let dark = dark();
        let bg = lightness(dark.background());
        assert!(lightness(dark.secondary()) > bg);
        assert!(lightness(dark.border()) > bg);
        assert!(lightness(dark.input()) > lightness(dark.border()));
        assert!(
            lightness(dark.base.colors.surface) > bg,
            "dark cards are lifted"
        );
        assert!(lightness(dark.foreground()) < lightness(dark.primary()) + 0.001);
        assert!(lightness(dark.muted_foreground()) < lightness(dark.foreground()));
    }

    #[test]
    fn tokens_reproduce_the_reference_values() {
        // Sampled from the reference application at contrast 45 (light) and
        // 60 (dark).
        let light = light();
        assert_eq!(to_hex(light.secondary()), "#f5f5f5");
        assert_eq!(to_hex(light.selected), "#e8e8e8");
        assert_eq!(to_hex(light.border()), "#ededed");
        assert_eq!(to_hex(light.input()), "#e5e5e6");
        assert_eq!(to_hex(light.muted_foreground()), "#67696b");
        let dark = dark();
        assert_eq!(to_hex(dark.secondary()), "#222222");
        assert_eq!(to_hex(dark.base.colors.surface), "#232323");
        assert_eq!(to_hex(dark.selected), "#2c2c2c");
        assert_eq!(to_hex(dark.border()), "#252525");
        assert_eq!(to_hex(dark.foreground()), "#dfdfdf");
        assert_eq!(to_hex(dark.muted_foreground()), "#969696");
        assert_eq!(to_hex(dark.primary()), "#dfdfdf");
        assert_eq!(to_hex(dark.primary_foreground()), "#2d2d2d");
        assert_eq!(to_hex(dark.tooltip), "#1b1b1b");
        assert_eq!(to_hex(dark.tooltip_foreground), "#ffffff");
        assert_eq!(to_hex(light.tooltip), "#1a1c1f");
        assert_eq!(to_hex(light.foreground()), "#1a1c1f");
    }

    #[test]
    fn contrast_scales_the_distance() {
        let mut low = ThemeConfig::light();
        low.contrast = 20;
        let mut high = ThemeConfig::light();
        high.contrast = 80;
        let low = ThemeTokens::derive(&low, ThemeAppearance::Light, metrics());
        let high = ThemeTokens::derive(&high, ThemeAppearance::Light, metrics());
        assert!(lightness(low.secondary()) > lightness(high.secondary()));
        assert!(lightness(low.selected) > lightness(high.selected));
        assert_eq!(
            low.border(),
            high.border(),
            "separators do not move with contrast"
        );
    }

    #[test]
    fn accent_and_semantic_colors_pass_through() {
        let light = light();
        assert_eq!(light.ring(), hex("#339cff"));
        assert_eq!(light.destructive(), hex("#ba2623"));
        assert_eq!(light.destructive_foreground(), hex("#ffffff"));
        assert_eq!(light.selection().a, 0.3);
        let dark = dark();
        assert_eq!(dark.ring(), hex("#539af8"));
        assert_eq!(
            to_hex(dark.link),
            "#91bffd",
            "links are paler than the ring on dark"
        );
        assert_eq!(light.link, light.ring(), "and the accent itself on light");
        assert_eq!(dark.destructive_foreground(), hex("#ffffff"));
        let mut pale = ThemeConfig::light();
        pale.semantic.destructive = hex("#ffe066");
        let pale = ThemeTokens::derive(&pale, ThemeAppearance::Light, metrics());
        assert_eq!(
            pale.destructive_foreground(),
            hex("#1a1c1f"),
            "ink reads on pale yellow"
        );
    }

    #[test]
    fn radius_scale_derives_from_one_value() {
        let tokens = light();
        assert_eq!(tokens.radius_sm(), px(6.));
        assert_eq!(tokens.radius_md(), px(8.));
        assert_eq!(tokens.radius_lg(), px(10.));
        assert_eq!(tokens.radius_xl(), px(14.));
        assert_eq!(tokens.radius_full(), px(9999.));
    }

    #[test]
    fn typography_follows_the_font_sizes_and_families() {
        let mut config = ThemeConfig::light();
        config.fonts.ui = Some("Inter".into());
        config.fonts.code = Some("JetBrains Mono".into());
        let tokens = ThemeTokens::derive(
            &config,
            ThemeAppearance::Light,
            Metrics {
                radius: px(10.),
                ui_font_size: px(20.),
                code_font_size: px(15.),
            },
        );
        assert_eq!(tokens.font_family(), &SharedString::from("Inter"));
        assert_eq!(
            tokens.mono_font_family(),
            &SharedString::from("JetBrains Mono")
        );
        assert_eq!(tokens.base.typography.md.size, px(20.));
        assert_eq!(tokens.base.typography.sm.size, px(17.5));
        assert_eq!(tokens.base.typography.md.line_height, px(30.));
        assert_eq!(tokens.base.typography.mono_md.size, px(15.));
        let defaults =
            ThemeTokens::derive(&ThemeConfig::light(), ThemeAppearance::Light, metrics());
        assert_eq!(defaults.font_family(), &TypographyTokens::default().sans);
        assert_eq!(defaults.base.typography.md.size, px(16.));
        assert_eq!(defaults.base.typography.mono_md.size, px(13.));
    }
}
