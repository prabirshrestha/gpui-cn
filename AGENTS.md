# AGENTS.md

Read this file before you change anything in this repository. The README
tells users what the library does. This file tells you how to build it.

## What this repository is

gpui-cn is a shadcn-style component library for GPUI. It is built on
`gpui-base` only, and its look is measured from a reference desktop app
that this repository does not name. The workspace has three crates:

- `crates/gpui-cn` is the library. Never add `gpui-component` or use a
  `gpui-component` type.
- `crates/gpui-cn-story` is the gallery. It uses gpui-cn components only,
  which makes it the first application that uses the library. Its
  `snapshot` binary renders the gallery to PNG files without a window, on
  macOS. `cargo run` at the root opens it.

## Rules

1. `gpui-base` owns behavior and gpui-cn owns the look. Focus, keyboard
   activation, the disabled state, accessibility roles, tooltips, motion,
   the navigation stack, collapsible regions, and drag all come from
   `gpui_kit::base`. Before you write behavior, search `gpui-base` for it
   in `~/.cargo/registry/src/*/gpui-base-*/src`. Use `h_flex()` and
   `v_flex()` from `gpui-base` for flex rows and columns.
2. Use tokens, not literal values. No component holds its own size, color,
   or text size. Read colors from `cx.theme()`, sizes from
   `theme.metrics`, and text styles from `theme.text_control`,
   `theme.text_heading`, `theme.text_title`, and `theme.base.typography`.
   The rem helpers such as `px_2`, `gap_1`, and `size_4` are the spacing
   scale, and a 1px hairline is fine. Any other value is a token. Add it to
   `crates/gpui-cn/src/theme/tokens.rs` with a doc comment that says where
   the value comes from.
3. Measure the reference app. Do not guess. To make a visual decision,
   capture its window with `screencapture -l <window id>`, sample the
   pixels, and record the value in the token's doc comment and in a test
   that asserts the derived hex. The theme derives every color from
   surface, ink, accent, and contrast, so add a derivation, not a literal
   color. A value that comes from shadcn instead, such as the sheet
   breakpoint, says so in its doc comment.
4. Control and row sizes scale with the UI font size, on the rem scale.
   The title bar, the window controls, and the sidebar widths are fixed
   pixels. Keep that split.
5. Anything that scrolls vertically is a `ScrollArea`, never a bare
   `overflow_y_scroll`. GPUI hands a wheel step to every scroll region
   under the pointer, so a bare region inside a page scrolls the page
   with it. `ScrollArea` keeps the step while it has room to scroll and
   passes it on when its content fits, and it bounces at its ends. A
   region that shows or hides content animates with `transition` from
   `gpui_base` and the theme's motion; nothing appears or vanishes in one
   frame unless motion is reduced.
6. Every component works with touch. A tap is a mouse down and up at one
   point, so nothing may depend on a hover that came first, and a hover
   state paints as rest while `theme.touch` is set. Sizes come from
   `theme.metrics`, which already hold the 44pt hit targets on touch. Vertical
   scrolling goes through `ScrollArea`, which bounces at its ends on every
   platform. A layout that changes shape on a phone, such as the sidebar
   becoming a sheet, decides from the window width, never from the
   target OS.
7. Public API is builders and readers. Components have no `pub` fields.
   A builder takes `impl Into<...>`. A reader is a plain name such as
   `width()`. A builder that seeds a value is `with_width(..)`. A mutation
   takes `cx`, such as `set_width(width, cx)`, and emits an event. Mark a
   struct that may grow `#[non_exhaustive]`.
8. Give repeated elements stable ids from the domain, never from a list
   index. Derive child ids with `ElementId::NamedChild`.
9. State lives in entities. A `RenderOnce` component reads an `Entity`,
   and the view that renders it observes that entity. Never store state
   in a render.
10. The gpui-kit guides are requirements. Read
   `https://gpui-kit.com/docs/coding-guides.md` and
   `https://gpui-kit.com/docs/design-guides.md`, and run the design review
   checklist before you finish UI work.

## Add a component

1. Read the nearest existing component. `button.rs` is a control and
   `sidebar.rs` is a composite. Copy its shape: a `#[derive(IntoElement)]`
   struct, a `new(id)` constructor, builders, `Styled`, `ParentElement`
   where children make sense, `Selectable` and `Disableable` from
   `gpui-base` where they apply, and a `RenderOnce` that reads the theme
   once.
2. Export it from `crates/gpui-cn/src/lib.rs` and add it to `prelude`.
3. Call `.test_support()` on the element that carries the component's id,
   before any `track_focus`, so tests can find it.
4. Write UI tests in `crates/gpui-cn/tests/<component>.rs` that drive a
   headless window with clicks, keys, and drags, and assert on bounds.
   Snapshots expose no disabled or selected flag, so prove those states
   by behavior.
5. Add a story in `crates/gpui-cn-story/src/stories/<component>.rs`.
   Register it in `stories/mod.rs` and in `stories()` in `lib.rs`, and add
   its title to the story list in `src/bin/snapshot.rs`.
6. Do not add it to the README, which stays at the hello world and the theme.

Fonts ship behind features, such as `jetbrains-mono`, and come from
crates. `theme/fonts.rs` registers them. The theme names font families,
never font files. Bundle a font only from a crate with a clear license.
Never copy a font file into the repository.

Icons come from the Lucide set in the gpui-kit-assets crate, as
`IconName` values. The application registers `gpui_kit::assets::Assets`,
which holds the names listed in `default-icons.txt` in that crate. Other
names render nothing unless the application registers
`gpui_kit::assets::AllAssets`. The composer family draws a few names the
default set lacks, so an application that uses it registers
`gpui_cn::ComposerAssets`, which adds them. Never copy an icon file into
the repository.

## Check a change

Every change must pass these commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc -p gpui-cn --no-deps --all-features
cargo deny check licenses
```

`deny.toml` lists the licenses this repository accepts. A crate that is
only available under the GPL or the LGPL fails the check. Never add one.

A change that touches layout, motion, or scrolling also runs the
benchmark and compares the numbers with the last run:

```bash
cargo run --release -p gpui-cn-story --features snapshot --bin bench
```

A frame of the heaviest page costs about 1.5 ms offscreen in release, and
an idle window costs no frames at all: GPUI draws only when something
changed. Keep both true. Never add a timer or an animation that runs
while nothing is visible.

Then look at the result:

```bash
cargo run -p gpui-cn-story --features snapshot --bin snapshot -- snapshots [height]
```

This writes PNG files for the gallery, the sidebar collapsed off the
canvas, the sidebar as a rail, the settings page, and each story, once per
appearance. A larger height shows a story that scrolls. Compare the files
with the reference app, not with memory.

## Conventions

- Use ASCII only in code, docs, and messages. Use plain dashes, never em
  dashes.
- A doc comment says what a thing is for and where its values come from,
  in short sentences.
- Do not keep old API for compatibility. Change the API and every call
  site.
- Commit messages follow Conventional Commits: `feat:`, `fix:`, `docs:`,
  `refactor:`, `test:`, or `chore:`, with an optional scope such as
  `feat(sidebar):`, a lower-case summary under 72 characters, and a body
  that says what changed and why. Never add an agent as a co-author.
- A new preference goes on the gallery's settings page. The shell's
  keyboard shortcuts and menu are in `crates/gpui-cn-story/src/main.rs`.

## Reference measurements

The values below come from the reference app. `theme/tokens.rs` derives them
and its tests assert them.

| Value | Dark | Light |
| --- | --- | --- |
| Sidebar | #222222 | #fdfdfd |
| Window | #181818 | #ffffff |
| Divider | #343434 | #ebebeb |
| Row, hovered | #2c2c2c | #f6f6f7 |
| Row, selected | #333333 | #f0f0f0 |
| Group label | #747474 | #a9aaab |
| Text | #dfdfdf | #1a1c1f |
| Muted text | #969696 | #67696b |
| Menu | #2d2d2d | derived |
| Menu hairline | #444444 | derived |
| Menu row, highlighted | #3d3d3d | derived |
| Menu separator | #3e3e3e | derived |
| Menu text | #ffffff | derived |
| Menu description | #b5b5b5 | derived |
| Menu check, trigger chevron | #cacaca | derived |
| Select trigger | #2a2a2a (derives #292929) | derived |
| Select trigger hairline | #3b3b3b (derives #3a3a3a) | derived |
| Field | #2c2c2c | #ffffff |
| Field border | #3b3b3b | #e5e5e6 |
| Field border, focused | #799cca (derived) | #339cff |

The reference app paints the hovered row and the selected row the same
color. gpui-cn keeps them one step apart so a hovered row beside the
selected row reads as two rows.

The title bar is 38px tall with its content 2px down and the macOS
window controls at 14px, measured from the tab reference app (the
reference app's own bar is 46px). On macOS the bar's content starts 88px
from the left edge, after the window controls. Rows are 30px. Controls are 28px
with 13px text. Text fields are 32px with 12px side padding, and a
textarea keeps 10px at the sides and 8px above and below its 18.5px
lines. The page title is 24px. The sidebar opens at 300px and
resizes between 220px and 480px.

A select menu opens 2px under its trigger, lined up with the trigger's
trailing edge, with 4px around its rows and the large radius. Its rows are
28px with the control padding, the check on the right, and the small
radius when highlighted. A search field is a 28px row at the top. A menu
with descriptions wraps at 360px, a searchable one is at least 240px wide,
and a menu scrolls past 390px. The light values are derived from the same
steps and were not sampled.
