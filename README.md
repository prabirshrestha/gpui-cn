# gpui-cn

shadcn-style components for [GPUI](https://gpui-kit.com), built on
`gpui-base`.

Base provides the behavior: focus, keyboard activation, accessibility,
tooltips, motion. gpui-cn adds the look: a small theme derived from a
surface, an ink, and an accent, and components styled from it. It depends on
`gpui-kit` with the component layer off, so it brings nothing into an
application beyond GPUI and `gpui-base`.

## Install

```toml
[dependencies]
gpui-kit = { version = "0.6", default-features = false }
gpui-cn = "0.1"
```

## Quick start

```rust
use gpui_cn::prelude::*;
use gpui_kit::*;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(
            Button::new("save")
                .primary()
                .label("Save")
                .on_click(|_, window, cx| {
                    // The receiver carries the chosen button; drop it for a
                    // plain notice.
                    let _ = window.prompt(PromptLevel::Info, "Saved", None, &["OK"], cx);
                }),
        )
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| Hello);
                cx.new(|cx| gpui_cn::Root::new(view, window, cx))
            })
            .expect("failed to open window");
        })
        .detach();
    });
}
```

`gpui_kit::init` sets up base; `gpui_cn::init` installs the theme on top of
it. `Root` paints the window from the theme, sets the rem size, handles Tab
and Shift-Tab, and hosts the tooltip overlay.

## Theme

The theme is a `ThemeConfig` per appearance: surface, ink, accent,
contrast, fonts, and semantic colors. Every token the components read is
derived from it and projected onto `gpui_base::Theme`, so base scrollbars,
inputs, and resize handles match.

```rust
Theme::change(ThemeMode::Dark, cx);
Theme::set_ui_font_size(cx, px(18.));
Theme::update(cx, |theme| {
    theme.light.accent = gpui_cn::theme::hex("#0f766e");
    theme.reduce_motion = ReduceMotion::On;
    theme.pointer_cursors = true;
});
```

Components read tokens with `cx.theme()`. Beside base's colors, radius,
spacing, and typography, the tokens carry what the reference application
adds: `text_control` (13px), `text_heading` (15px), `text_title` (24px), and
`metrics`, the sizes of controls, rows, and window chrome. Control and row
sizes follow the UI font size; the title bar, window controls, and sidebar
widths are fixed pixels. No component holds a size of its own, so a theme
that changes `metrics` changes every component.

## Window shell

`TitleBar` is a title bar the application draws: a drag region with room
for the platform's window controls, transparent so it takes the surface it
sits on. `SidebarLayout` puts a `Sidebar` beside the content, resizable by
its edge and collapsible to a rail of icons (the default) or off the
canvas. `NavStack`
shows the pages of a `gpui_base::NavStackState` and `NavButtons` are its
back and forward arrows. Together they make the shell the reference
application has: a sidebar with its own title bar, a content area with
another, and a settings page pushed over both.

```rust
SidebarLayout::new(&self.sidebar)
    .sidebar(
        Sidebar::new()
            .header(
                TitleBar::new()
                    .child(SidebarTrigger::new("trigger", &self.sidebar))
                    .child(NavButtons::new("nav", &self.stack)),
            )
            .child(
                SidebarGroup::new()
                    .label("Projects")
                    .action(Button::new("add").ghost().size(ButtonSize::Sm).icon(IconName::Plus))
                    .child(SidebarMenuButton::new("inbox").icon(IconName::Inbox).label("Inbox").badge("12"))
                    .child(SidebarMenuSub::new().child(SidebarMenuButton::new("today").label("Today"))),
            )
            .footer(SidebarMenuButton::new("settings").icon(IconName::Settings).label("Settings")),
    )
    .child(TitleBar::new().inset(!open).when(!open, |bar| bar.child(SidebarTrigger::new("t", &self.sidebar))))
    .child(page)
```

Open the window with `TitleBar::window_options(cx)` so the platform hides its
own title bar. `SidebarState` holds the open state, the width, its range,
and the collapse mode; share one between pages so the width survives
navigation. `NavStackExt::pop_and_discard` pops a page and drops it from the
forward history once its exit has run, so a page the application will not
bring back releases its state.

## Fonts

The interface font is the platform's. The code font is JetBrains Mono
when the `jetbrains-mono` feature is on: `init` registers the variable
font (from the `damascene-fonts-jetbrains-mono` crate, under the SIL Open
Font License) and the built-in themes name it. Without the feature the
code font is the platform's monospace. Either way an application names
any family the platform has:

```toml
gpui-cn = { version = "0.1", features = ["jetbrains-mono"] }
```

```rust
Theme::update(cx, |theme| theme.dark.fonts.code = Some("Menlo".into()));
```

## Icons

gpui-cn ships no icons. Pass your own SVGs:

```rust
Icon::new("icons/check.svg")
Icon::from_bytes(include_bytes!("check.svg"))
```

Or turn on the `assets` feature for the Lucide set that gpui-kit bundles:

```toml
gpui-cn = { version = "0.1", features = ["assets"] }
```

Then `Button::icon(IconName::Plus)` works once the application registers the
asset source with `gpui_kit::application().with_assets(gpui_kit::assets::Assets)`.

## In an existing application

Call `gpui_cn::init(cx)` after the host's own init (`gpui_kit::init` or
`gpui_component::init`). gpui-cn then owns the base theme the way
`gpui-component` does; when both run, the later init wins and the other
layer follows its colors. gpui-cn components work under any root view; without
`gpui_cn::Root` tooltips fall back to GPUI's native tooltip in the same
surface.

## Story

The story is a gallery of every component in every variant, size, and
state, in the shell the components build: a sidebar of stories under a
title bar, and a settings page (Cmd-, or the application menu) with light
and dark, reduce motion, pointer cursors, text size, and the sidebar's
collapse mode. It is the fastest way to see what a component looks like
and how it responds to the keyboard and the pointer.

```bash
cargo run -p gpui-cn-story
```

Pick a component in the sidebar; Cmd-B hides it and Cmd-[ and Cmd-] walk
the navigation history. Tab moves focus through the page and shows the
focus ring; hover an icon button or a truncated label for its tooltip.

On macOS the story can also render itself headless to PNG files (the
gallery, the collapsed sidebar, settings, and each story, per appearance)
for a visual check without a window:

```bash
cargo run -p gpui-cn-story --features snapshot --bin snapshot -- snapshots
```

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## License

Apache-2.0. Third-party assets and their licenses:

- Lucide icons (ISC), the SVGs in `crates/gpui-cn/icons`; the license is
  there as `LICENSE-LUCIDE`.
- JetBrains Mono (SIL Open Font License 1.1), through the
  `damascene-fonts-jetbrains-mono` crate with the `jetbrains-mono` feature;
  the crate carries the license text.

Every dependency in the tree is available under a permissive license
(Apache-2.0, MIT, BSD, ISC, Zlib, Unicode, MPL-2.0, CC0, or the OFL for the
font); the few dual-licensed crates that also offer the LGPL or GPL are
used under their permissive option.
