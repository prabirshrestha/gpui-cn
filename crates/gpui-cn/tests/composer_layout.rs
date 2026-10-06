//! The composer's toolbar at narrow and wide widths: its controls never
//! overlap, and long labels truncate.

use gpui_cn::{
    Composer, ComposerState, ModelEntry, ModelPickerState, ModelProvider, PermissionMode,
    PermissionState, ReduceMotion, Theme,
};
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, base::Root, div, px, size, test::TestWindowExt as _,
};

struct Harness {
    state: Entity<ComposerState>,
    width: f32,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(self.width))
            .child(Composer::new("composer", &self.state).no_status())
    }
}

fn part(name: &str) -> ElementId {
    ElementId::NamedChild(ElementId::from("composer").into(), name.to_string().into())
}

#[gpui_kit::test]
fn toolbar_controls_never_overlap_at_any_width(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    for width in [320., 360., 480., 720.] {
        let handle = cx.open_window(size(px(800.), px(400.)), |window, cx| {
            let models = cx.new(|cx| {
                ModelPickerState::new(
                    [ModelProvider::new("acme", "Acme").models([ModelEntry::new(
                        "long",
                        "Acme Extraordinarily Long Model Name Mini",
                    )
                    .effort(true)])],
                    window,
                    cx,
                )
            });
            let permission = cx.new(|cx| {
                PermissionState::new(cx).with_modes([PermissionMode::new(
                    "full",
                    "Full access with no prompts at all",
                )])
            });
            let state = cx.new(|cx| {
                ComposerState::new(window, cx)
                    .with_models(models, window, cx)
                    .with_permission(permission, window, cx)
            });
            let harness = cx.new(|_| Harness { state, width });
            Root::new(harness, window, cx)
        });
        cx.update_window(handle.into(), |_, window, _| window.activate_window())
            .unwrap();
        for _ in 0..2 {
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.render_frame(cx);
            })
            .unwrap();
        }
        cx.update_window(handle.into(), |_, window, _| {
            let card = window.find(part("card")).bounds();
            let found: Vec<_> = ["add", "permission", "models", "send"]
                .iter()
                .filter_map(|name| window.try_find(part(name)).map(|e| (*name, e.bounds())))
                .collect();
            assert!(found.len() >= 3, "controls found at {width}: {found:?}");
            for (at, (name, bounds)) in found.iter().enumerate() {
                assert!(
                    bounds.left() >= card.left() && bounds.right() <= card.right(),
                    "{name} inside the card at {width}: {bounds:?} {card:?}"
                );
                for (other, theirs) in &found[at + 1..] {
                    assert!(
                        !bounds.intersects(theirs),
                        "{name} overlaps {other} at {width}: {bounds:?} {theirs:?}"
                    );
                }
            }
        })
        .unwrap();
    }
}
