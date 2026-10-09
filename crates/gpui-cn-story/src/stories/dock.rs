use gpui_cn::{
    ActiveTheme as _, Button, DockSkin,
    dock::{DockArea, DockLayout, Panel, PanelEvent},
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, Hsla, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render,
    Styled as _, Window, div, px,
};

use crate::{Story, frame, note, page, section};

/// Which status fill a pane of the story shows, so the panes tell apart
/// at a glance while one moves.
#[derive(Clone, Copy)]
enum Tint {
    Info,
    Success,
    Warning,
    Destructive,
}

/// A plain pane: a name on a tinted fill. A press focuses it, so the
/// focus ring follows the pointer as it does in the Terminal story.
struct ColorPane {
    name: &'static str,
    tint: Tint,
    focus: FocusHandle,
}

impl ColorPane {
    fn new(name: &'static str, tint: Tint, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            name,
            tint,
            focus: cx.focus_handle(),
        })
    }

    fn fill(&self, cx: &App) -> Hsla {
        let theme = cx.theme();
        match self.tint {
            Tint::Info => theme.info_tint,
            Tint::Success => theme.success_tint,
            Tint::Warning => theme.warning_tint,
            Tint::Destructive => theme.destructive_tint,
        }
    }
}

impl Panel for ColorPane {
    fn panel_name(&self) -> &'static str {
        self.name
    }
}

impl EventEmitter<PanelEvent> for ColorPane {}

impl Focusable for ColorPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ColorPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let focus = self.focus.clone();
        div()
            .id(ElementId::Name(self.name.into()))
            .track_focus(&self.focus)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focus.focus(window, cx);
            })
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .bg(self.fill(cx))
            .child(
                div()
                    .text_size(theme.text_control.size)
                    .text_color(theme.foreground())
                    .child(self.name),
            )
    }
}

/// Panes in splits that the user rearranges by their grab handles.
pub struct DockStory {
    area: Entity<DockArea>,
    panes: [Entity<ColorPane>; 4],
}

impl Story for DockStory {
    fn title() -> &'static str {
        "Dock"
    }

    fn icon() -> IconName {
        IconName::LayoutDashboard
    }

    fn description() -> &'static str {
        "Panes in splits. Drag a pane by the handle at its top to move it beside another."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let panes = [
                ColorPane::new("Files", Tint::Info, cx),
                ColorPane::new("Editor", Tint::Success, cx),
                ColorPane::new("Console", Tint::Destructive, cx),
                ColorPane::new("Preview", Tint::Warning, cx),
            ];
            let area = DockSkin::area("dock-story", window, cx);
            cx.observe(&area, |_, _, cx| cx.notify()).detach();
            let mut story = Self { area, panes };
            story.reset(window, cx);
            story
        })
        .into()
    }
}

impl DockStory {
    /// Puts the panes back where the story starts them.
    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [files, editor, console, preview] = self.panes.clone();
        self.area.update(cx, |area, cx| {
            area.set_center(
                DockLayout::h_split()
                    .child(DockLayout::tabs().panel(files), Some(px(200.)))
                    .child(
                        DockLayout::v_split()
                            .child(DockLayout::tabs().panel(editor), None)
                            .child(DockLayout::tabs().panel(console), Some(px(140.))),
                        None,
                    )
                    .child(DockLayout::tabs().panel(preview), Some(px(220.))),
                window,
                cx,
            );
        });
    }
}

impl Render for DockStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        page([section(
            "Rearrange",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .child(note(
                    "Point at the top of a pane to show its handle, then drag it over \
                         another pane. After a short rest the panes make room; let go to keep \
                         it there, or press Escape to put everything back.",
                    cx,
                ))
                .child(frame(px(420.), cx).child(self.area.clone()))
                .child(
                    div().flex().child(
                        Button::new("reset")
                            .outline()
                            .label("Reset layout")
                            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                    ),
                ),
        )])
    }
}
