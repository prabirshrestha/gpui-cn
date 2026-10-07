use gpui_cn::{Radio, RadioGroup, gpui_kit::assets::IconName};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
    Window, base::Disableable as _,
};

use crate::{Story, note, page, section};

/// A radio group the story drives, a row of radios, and a disabled one.
pub struct RadioStory {
    size: &'static str,
    plan: &'static str,
}

impl Story for RadioStory {
    fn title() -> &'static str {
        "Radio"
    }

    fn icon() -> IconName {
        IconName::Check
    }

    fn description() -> &'static str {
        "A set of checkable buttons where no more than one can be checked at a time."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self {
            size: "medium",
            plan: "monthly",
        })
        .into()
    }
}

impl Render for RadioStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.size;
        let plan = self.plan;
        let sizes = ["small", "medium", "large"].map(|name| {
            Radio::new(name)
                .label(name)
                .checked(size == name)
                .on_change(cx.listener(move |this, _, _, cx| {
                    this.size = name;
                    cx.notify();
                }))
        });
        let plans = ["monthly", "yearly"].map(|name| {
            Radio::new(name)
                .label(name)
                .checked(plan == name)
                .on_change(cx.listener(move |this, _, _, cx| {
                    this.plan = name;
                    cx.notify();
                }))
        });
        page([
            section(
                "Group",
                gpui_kit::div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The choice is the story's own. Tab to a radio and press Space or Enter \
                         to choose it.",
                        cx,
                    ))
                    .child(RadioGroup::new("size").children(sizes)),
            )
            .into_any_element(),
            section(
                "Horizontal",
                RadioGroup::new("plan").horizontal().children(plans),
            )
            .into_any_element(),
            section(
                "Disabled",
                RadioGroup::new("locked")
                    .child(
                        Radio::new("locked-on")
                            .label("Chosen")
                            .checked(true)
                            .disabled(true),
                    )
                    .child(Radio::new("locked-off").label("Not chosen").disabled(true)),
            )
            .into_any_element(),
        ])
    }
}
