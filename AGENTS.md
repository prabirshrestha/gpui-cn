# AGENTS.md

Instructions for coding agents working in this repository. Read this file
before changing anything. The README explains the library to its users;
this file explains how the library is built.

## What this is

gpui-cn is a shadcn-style component library for GPUI, built on
`gpui-base` only. Its look is measured from the ChatGPT (Codex) desktop
application. Two crates:

- `crates/gpui-cn`: the library. Depends on `gpui-kit` with the component
  layer off, so it brings GPUI and `gpui-base` and nothing else. Never add
  `gpui-component` or a `gpui-component` type.
- `crates/gpui-cn-story`: the gallery. It is the first consumer of the
  library and must be built from gpui-cn components only. It has a
  `snapshot` binary that renders the gallery headless to PNG on macOS.

## The rules

1. **Base owns behavior, gpui-cn owns the look.** Focus, keyboard
   activation, disabled contract, accessibility roles, tooltips, motion
   sampling, the navigation stack, collapsible regions, and drag come from
   `gpui_kit::base`. Before writing behavior, search base for it
   (`~/.cargo/registry/src/*/gpui-base-*/src`). Use `h_flex()` and
   `v_flex()` from base for flex rows and columns.
2. **Tokens before values.** No component holds a size, color, or text
   size of its own. Colors come from `cx.theme()` (`ThemeTokens`), sizes
   from `theme.metrics` (`MetricTokens`), text from `theme.text_control`,
   `theme.text_heading`, `theme.text_title`, and `theme.base.typography`.
   Rem-scale helpers such as `px_2`, `gap_1`, `size_4` are the spacing
   scale and are fine. A 1px hairline is fine. Anything else is a token;
   add it to `crates/gpui-cn/src/theme/tokens.rs` with a doc comment that
   says where the value comes from.
3. **Measure, do not guess.** A visual decision comes from the reference
   application: capture its window (`screencapture -l <window id>`),
   sample pixels, and record the measurement in the token's doc comment
   and in a test that asserts the derived hex. The theme derives every
   color from surface, ink, accent, and contrast; add a derivation, not a
   literal color.
4. **Sizes scale with the UI font size.** Control and row sizes are on the
   rem scale. Window chrome (title bar, window controls) and layout widths
   (sidebar) are fixed pixels. Keep that split.
5. **Public API is builders and readers.** No `pub` fields on components.
   Builders take `impl Into<...>`; readers are plain names (`width()`),
   builders that seed a value are `with_...` (`with_width(..)`), and
   mutations take `cx` (`set_width(width, cx)`) and emit an event.
   Structs that may grow are `#[non_exhaustive]`.
6. **Stable ids.** Repeated elements take domain ids, never indexes into a
   list. Child elements derive ids with `ElementId::NamedChild`.
7. **Everything re-renders through entities.** A `RenderOnce` component
   reads `Entity` state; the view that renders it observes that entity.
   Never store state in a render.
8. **Read the guides.** The gpui-kit coding and design guides are
   requirements: `https://gpui-kit.com/docs/coding-guides.md` and
   `https://gpui-kit.com/docs/design-guides.md`. Run the design review
   checklist before finishing UI work.

## Adding a component

1. Read the nearest existing component (`button.rs` for a control,
   `sidebar.rs` for a composite) and copy its shape: a `#[derive(IntoElement)]`
   struct, a `new(id)` constructor, builders, `Styled`, `ParentElement`
   where children make sense, `Selectable` and `Disableable` from base
   where they apply, and a `RenderOnce` that reads the theme once.
2. Export it from `crates/gpui-cn/src/lib.rs` and add it to `prelude`.
3. Observe it for tests with `.test_support()` on the element that carries
   its id (before any `track_focus`).
4. Write UI tests in `crates/gpui-cn/tests/<component>.rs` through a
   headless window: real clicks, keys, drags, and bounds. Base exposes no
   disabled or selected flag in snapshots; prove those by behavior.
5. Add a story in `crates/gpui-cn-story/src/stories/<component>.rs`, register
   it in `stories/mod.rs` and in `stories()` in `lib.rs`, and add its title
   to the snapshot binary's story list.
6. Describe it in the README.

Fonts ship behind features (`jetbrains-mono`, from a crate) and are
registered in `theme/fonts.rs`; the theme names families, never files.
Bundle a font only from a crate with a clear license; never vendor one.

Icons a component needs come as SVG bytes in `crates/gpui-cn/icons` (Lucide,
see `LICENSE-LUCIDE` there) through `Icon::from_bytes`, so the component
works without the `assets` feature. The gallery may use `IconName`, but only
names in gpui-kit-assets' `default-icons.txt`; others render nothing.

## Checks

Every change passes all of these before it is done:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc -p gpui-cn --no-deps --all-features
```

Then look at the result:

```bash
cargo run -p gpui-cn-story --features snapshot --bin snapshot -- snapshots [height]
```

writes the gallery, the collapsed sidebar, settings, and every story per
appearance. A taller height shows a story that scrolls. Compare against the
reference application, not against memory.

## Conventions

- ASCII only in code, docs, and messages. Plain dashes, no em dashes.
- Doc comments say what a thing is for and where its values come from, in
  short sentences.
- No backward-compatibility shims: change the API and every call site.
- Commit messages describe the change; never add an agent as co-author.
- The gallery's settings page is where a preference goes; the shell's
  keyboard shortcuts and menu are in `crates/gpui-cn-story/src/main.rs`.

## Reference measurements

Dark: sidebar #222222, window #181818, divider #343434, row hover #2c2c2c,
selected #333333 (the reference paints both #333333), group label #747474,
text #dfdfdf, muted #969696.
Light: sidebar #fdfdfd, window #ffffff, divider #ebebeb, row #f0f0f0.
Title bar 46px with controls starting at 88px on macOS; rows 30px;
controls 28px with 13px text; page title 24px; sidebar 300px, 220px to
480px. These are derived in `theme/tokens.rs` and asserted in its tests.
