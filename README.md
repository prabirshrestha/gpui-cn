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

Components read tokens with `cx.theme()`.

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
state, with switches for light and dark, reduce motion, pointer cursors,
and text size. It is the fastest way to see what a component looks like
and how it responds to the keyboard and the pointer.

```bash
cargo run -p gpui-cn-story
```

Pick a component in the sidebar. Tab moves focus through the page and
shows the focus ring; hover an icon button or a truncated label for its
tooltip.

On macOS the story can also render itself headless to PNG files, one per
appearance, for a visual check without a window:

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

Apache-2.0
