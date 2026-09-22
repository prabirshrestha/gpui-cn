use gpui_cn::{
    ActiveTheme as _, Button, Icon, Input, InputEvent, InputState, Label,
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window,
    base::{Disableable as _, h_flex, v_flex},
    div,
};

use crate::{Story, note, page, section};

/// The single-line text field in every state.
pub struct InputStory {
    name: Entity<InputState>,
    search: Entity<InputState>,
    command: Entity<InputState>,
    password: Entity<InputState>,
    disabled: Entity<InputState>,
    readonly: Entity<InputState>,
    submit: Entity<InputState>,
    submitted: Option<SharedString>,
    email: Entity<InputState>,
    full_name: Entity<InputState>,
}

impl Story for InputStory {
    fn title() -> &'static str {
        "Input"
    }

    fn icon() -> IconName {
        IconName::CaseSensitive
    }

    fn description() -> &'static str {
        "Displays a form input field or a component that looks like an input field."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let submit = cx.new(|cx| InputState::new(window, cx).placeholder("Press Enter"));
            cx.subscribe(&submit, |this: &mut Self, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submitted = Some(state.read(cx).value());
                    cx.notify();
                }
            })
            .detach();
            Self {
                name: cx.new(|cx| InputState::new(window, cx).placeholder("Name")),
                search: cx.new(|cx| InputState::new(window, cx).placeholder("Search")),
                command: cx.new(|cx| InputState::new(window, cx).placeholder("Search")),
                password: cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Password")
                        .masked(true)
                        .default_value("hunter2")
                }),
                disabled: cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Disabled")
                        .default_value("Cannot edit")
                }),
                readonly: cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Read only")
                        .default_value("Select and copy, but do not edit")
                }),
                submit,
                submitted: None,
                email: cx.new(|cx| InputState::new(window, cx).placeholder("you@example.com")),
                full_name: cx.new(|cx| InputState::new(window, cx).placeholder("Ada Lovelace")),
            }
        })
        .into()
    }
}

/// The width a story field takes, so a field does not run the page.
const FIELD_WIDTH: gpui_kit::Pixels = gpui_kit::px(320.);

impl Render for InputStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground();
        let field = |input: Input| div().w(FIELD_WIDTH).child(input);
        page([
            section("Default", field(Input::new(&self.name).id("name"))).into_any_element(),
            section(
                "Search",
                v_flex()
                    .gap_3()
                    .child(note(
                        "A clear button appears while the field holds a value.",
                        cx,
                    ))
                    .child(field(
                        Input::new(&self.search)
                            .id("search")
                            .cleanable(true)
                            .prefix(Icon::from(IconName::Search).text_color(muted)),
                    )),
            )
            .into_any_element(),
            section(
                "Password",
                field(Input::new(&self.password).mask_toggle(true)),
            )
            .into_any_element(),
            section(
                "With prefix and suffix",
                field(
                    Input::new(&self.command)
                        .prefix(Icon::from(IconName::Search).text_color(muted))
                        .suffix(div().text_color(muted).child("Enter")),
                ),
            )
            .into_any_element(),
            section("Disabled", field(Input::new(&self.disabled).disabled(true)))
                .into_any_element(),
            section(
                "Read only",
                field(Input::new(&self.readonly).readonly(true)),
            )
            .into_any_element(),
            section(
                "In a form",
                v_flex()
                    .gap_6()
                    .w(FIELD_WIDTH)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("email-label", "Email").for_field(&self.email))
                            .child(Input::new(&self.email)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(Label::new("name-label", "Name").for_field(&self.full_name))
                            .child(Input::new(&self.full_name).flex_1())
                            .child(Button::new("save").primary().label("Save")),
                    ),
            )
            .into_any_element(),
            section(
                "Submit",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Enter reports a submit through InputEvent::PressEnter.",
                        cx,
                    ))
                    .child(field(Input::new(&self.submit)))
                    .child(note(
                        match &self.submitted {
                            Some(value) if !value.is_empty() => {
                                SharedString::from(format!("Submitted: {value}"))
                            }
                            Some(_) => "Submitted an empty value".into(),
                            None => "Nothing submitted yet".into(),
                        },
                        cx,
                    )),
            )
            .into_any_element(),
        ])
    }
}
