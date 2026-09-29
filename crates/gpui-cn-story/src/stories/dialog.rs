use gpui_cn::{Button, Dialog, Input, InputState, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window,
};

use crate::{Story, note, page, section};

/// A modal window over the page.
pub struct DialogStory {
    open: bool,
    name: Entity<InputState>,
    focus: FocusHandle,
}

impl DialogStory {
    /// The button that opens the dialog.
    pub const TRIGGER: &'static str = "dialog-trigger";
    /// The button that closes the dialog with its footer.
    pub const SAVE: &'static str = "dialog-save";

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        cx.notify();
    }
}

impl Story for DialogStory {
    fn title() -> &'static str {
        "Dialog"
    }

    fn icon() -> IconName {
        IconName::WindowMaximize
    }

    fn description() -> &'static str {
        "A modal window with a title, a description, a body, and a footer."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
            let focus = name.focus_handle(cx);
            Self {
                open: false,
                name,
                focus,
            }
        })
        .into()
    }
}

impl Render for DialogStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let update = move |open: bool| {
            let this = this.clone();
            move |cx: &mut App| {
                this.update(cx, |this, cx| this.set_open(open, cx)).ok();
            }
        };
        let (open, close, save) = (update(true), update(false), update(false));
        page([section(
            "Edit profile",
            gpui_kit::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(
                    "The button opens the dialog with the name field focused. Escape, a \
                     press on the backdrop, or the close button closes it, and focus goes \
                     back to the button.",
                    cx,
                ))
                .child(
                    gpui_kit::div().flex().child(
                        Button::new(Self::TRIGGER)
                            .label("Edit profile")
                            .on_click(move |_, _, cx| open(cx)),
                    ),
                )
                .child(
                    Dialog::new("dialog-profile")
                        .open(self.open)
                        .title("Edit profile")
                        .description("Make changes to your profile here.")
                        .track_focus(&self.focus)
                        .on_open_change(move |_, _, cx| close(cx))
                        .child(Input::new(&self.name))
                        .footer(
                            gpui_kit::div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("dialog-cancel")
                                        .ghost()
                                        .label("Cancel")
                                        .on_click({
                                            let cancel = update(false);
                                            move |_, _, cx| cancel(cx)
                                        }),
                                )
                                .child(
                                    Button::new(Self::SAVE)
                                        .primary()
                                        .label("Save changes")
                                        .on_click(move |_, _, cx| save(cx)),
                                ),
                        ),
                ),
        )
        .into_any_element()])
    }
}
