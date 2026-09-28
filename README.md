# gpui-cn

shadcn-style components for GPUI, built on the `gpui-base` layer of
[gpui-kit](https://gpui-kit.com/).

`gpui-base` owns the behavior: focus, keyboard activation, accessibility,
tooltips, and motion. gpui-cn owns the look: a theme derived from a
surface color, an ink color, and an accent color, and components styled
from that theme.

## Install

```bash
cargo add gpui-kit gpui-cn
```

This adds the two dependencies to `Cargo.toml`:

```toml
[dependencies]
gpui-kit = "0.6"
gpui-cn = "0.1"
```

## Show a button

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
                    let _ = window.prompt(PromptLevel::Info, "Saved", None, &["OK"], cx);
                }),
        )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_cn::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Hello))
                .expect("failed to open window");
        });
}
```

`with_assets` registers the Lucide icons that gpui-kit bundles, which the
components draw with. Call `gpui_kit::init` first. It sets up `gpui-base`.
Then call `gpui_cn::init`, which installs the theme and adds gpui-cn to
the `Root` that `gpui_kit::open_window` puts at the top of each window.
gpui-base's root moves focus on Tab and Shift-Tab and copies the selected
text on Cmd-C; gpui-cn paints the window background from the theme, sets
the rem size, and hosts the tooltip overlay.

## Theme

A `ThemeConfig` holds the inputs for one appearance: surface, ink, accent,
contrast, fonts, and semantic colors. `Theme` holds one config for light
and one for dark. Every token a component reads is derived from the active
config and copied onto `gpui_base::Theme`, so the scrollbars, inputs, and
resize handles from `gpui-base` use the same colors.

```rust
Theme::change(ThemeMode::Dark, cx);
Theme::set_ui_font_size(cx, px(18.));
Theme::update(cx, |theme| {
    theme.light.accent = gpui_cn::theme::hex("#0f766e");
    theme.reduce_motion = ReduceMotion::On;
    theme.pointer_cursors = true;
});
```

Components read tokens with `cx.theme()`. The tokens hold the colors,
radii, spacing, and typography from `gpui-base`, plus:

- `text_control`, `text_heading`, and `text_title`: 13px, 15px, and 24px
  text at the default UI font size.
- `metrics`: the heights and paddings of controls, the heights of rows,
  the title bar height, the space the window controls take, and the
  sidebar widths.

Control and row sizes scale with the UI font size. The title bar, the
window controls, and the sidebar widths stay in pixels. No component holds
a size of its own, so a theme that changes `metrics` changes every
component. `theme.touch`, on by default on iOS and Android, grows controls
and rows to the 44pt hit target of Apple's Human Interface Guidelines,
sets text to its body size, and turns hover states off.

## Gallery

The gallery shows every component in every variant, size, and state. It
is built from gpui-cn components, so it is also the first application that
uses the library.

```bash
cargo run
```

Pick a story in the sidebar. Each story you pick is a page on the
navigation stack, and the arrows in the title bar walk back and forward
through them. Tab moves focus through the page and shows the focus ring.
Hover an icon button or a truncated label to see its tooltip. In a window
narrower than 768px the sidebar becomes a sheet over the page, as on a
phone.

| Key | Action |
| --- | --- |
| Cmd-, | Open Settings: appearance, reduced motion, pointer cursors, text size, code font, and the sidebar's collapse mode |
| Cmd-B | Hide or show the sidebar |
| Cmd-[ | Go back |
| Cmd-] | Go forward |

On Windows and Linux, the same actions use Ctrl-, Ctrl-B, Alt-Left, and
Alt-Right.

On macOS, the gallery can also render itself without a window and write
PNG files: the gallery, the sidebar collapsed off the canvas, the sidebar
as a rail, the settings page, and each story, once per appearance. Pass a
height to see a story that scrolls.

```bash
cargo run -p gpui-cn-story --features snapshot --bin snapshot -- snapshots
cargo run -p gpui-cn-story --features snapshot --bin snapshot -- snapshots 2400
```

## Performance

Two tools measure the gallery:

```bash
cargo run --release -p gpui-cn-story --features snapshot --bin bench
cargo run --release --features fps -- --exercise
```

`GPUI_CN_STARTUP_TRACE=1` makes the gallery print when the application
was built, when the window existed, and when the first frame drew, in
milliseconds since `main`. `bench` renders every page offscreen and prints what a full frame, an
idle frame, a wheel step, and the sidebar's slide cost, with the resident
memory before and after the slide. The `fps` feature adds the
[gpui-fps](https://crates.io/crates/gpui-fps) HUD, toggled with
Cmd-Shift-P or **View > Performance HUD**, which reads GPUI's own frame
trace. `--exercise` drives the gallery by itself (scroll, slide, next
story) so a profiler such as Instruments can watch it.

## Development

Every change must pass these commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc -p gpui-cn --no-deps --all-features
cargo deny check licenses
```

`AGENTS.md` describes how the library is built and the rules a change
must follow.

To release, set the version in `Cargo.toml`, commit, and push a tag with
the same version (`v0.1.0`). The `Release` workflow checks that the tag
matches and runs `cargo publish -p gpui-cn`. Only `gpui-cn` is published;
the gallery crates are marked `publish = false`.

## License

Apache-2.0. Third-party assets:

- The Lucide icons come from the `gpui-kit-assets` crate under the ISC
  license.
- JetBrains Mono, used with the `jetbrains-mono` feature, comes from the
  `damascene-fonts-jetbrains-mono` crate under the SIL Open Font License
  1.1.
