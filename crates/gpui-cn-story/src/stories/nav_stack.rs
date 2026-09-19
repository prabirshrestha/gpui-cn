use gpui_cn::{
    ActiveTheme as _, Button, ButtonSize, NavButtons, NavMotion, NavStack, NavStackExt as _,
    NavStackState, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, WeakEntity, Window, base::StyledExt as _, div, px,
};

use crate::{Story, frame, note, page, section};

/// A navigation stack in a frame, with the arrows that drive it.
pub struct NavStackStory {
    stack: Entity<NavStackState>,
    pushed: usize,
    /// Every page ever pushed, weakly, to show which are still alive.
    pages: Vec<WeakEntity<DemoPage>>,
}

impl Story for NavStackStory {
    fn title() -> &'static str {
        "Nav stack"
    }

    fn icon() -> IconName {
        IconName::GalleryVerticalEnd
    }

    fn description() -> &'static str {
        "Pages pushed over each other, with back and forward through the history."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let stack = cx.new(|_| NavStackState::new());
            let root = cx.new(|_| DemoPage { number: 1 });
            let pages = vec![root.downgrade()];
            stack.update(cx, |stack, cx| {
                stack.push(root, NavMotion::Immediate, cx);
            });
            cx.observe(&stack, |_, _, cx| cx.notify()).detach();
            Self {
                stack,
                pushed: 1,
                pages,
            }
        })
        .into()
    }
}

/// One page of the demo: its number, on a surface that alternates.
struct DemoPage {
    number: usize,
}

impl Render for DemoPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let odd = self.number % 2 == 1;
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .gap_1()
            .bg(if odd {
                theme.background()
            } else {
                theme.secondary()
            })
            .child(
                div()
                    .text_lg()
                    .font_medium()
                    .child(format!("Page {}", self.number)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground())
                    .child("Pushed with a crossfade; back and forward keep the history."),
            )
    }
}

impl NavStackStory {
    /// Pushes the next numbered page.
    pub fn push_page(&mut self, cx: &mut Context<Self>) {
        self.pushed += 1;
        let number = self.pushed;
        let page = cx.new(|_| DemoPage { number });
        self.pages.push(page.downgrade());
        self.stack.update(cx, |stack, cx| {
            stack.push(page, NavMotion::Animated, cx);
        });
    }

    /// Pops the top page. It waits in the forward history, as Back does.
    pub fn pop_page(&mut self, cx: &mut Context<Self>) {
        self.stack.update(cx, |stack, cx| {
            stack.pop(NavMotion::Animated, cx);
        });
    }

    /// Pops the top page and drops it once its exit has run.
    pub fn pop_and_discard(&mut self, cx: &mut Context<Self>) {
        self.stack.pop_and_discard(NavMotion::Animated, cx);
    }
}

impl Render for NavStackStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (depth, forward) = {
            let stack = self.stack.read(cx);
            (stack.depth(), stack.forward_views().len())
        };
        let muted = cx.theme().muted_foreground();
        let title_bar = cx.theme().metrics.title_bar;
        self.pages.retain(|page| page.upgrade().is_some());
        let alive = self.pages.len();
        page([section(
            "Stack",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .child(note(
                    "Push adds a page and drops the forward history, as a browser does. Pop \
                         and the back arrow are the same: the page waits in the forward \
                         history, and the root stays. Pop and discard drops the page once its \
                         exit has run, so nothing keeps it alive.",
                    cx,
                ))
                .child(
                    frame(px(320.), cx).child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .h(title_bar)
                                    .px_2()
                                    .child(NavButtons::new("story-nav", &self.stack))
                                    .child(
                                        Button::new("push")
                                            .size(ButtonSize::Sm)
                                            .label("Push page")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.push_page(cx)),
                                            ),
                                    )
                                    .child(
                                        Button::new("pop")
                                            .ghost()
                                            .size(ButtonSize::Sm)
                                            .label("Pop page")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.pop_page(cx)),
                                            ),
                                    )
                                    .child(
                                        Button::new("pop-discard")
                                            .ghost()
                                            .size(ButtonSize::Sm)
                                            .label("Pop and discard")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.pop_and_discard(cx)
                                            })),
                                    )
                                    .child(div().flex_1())
                                    .child(div().text_xs().text_color(muted).child(format!(
                                        "Depth {depth}, {forward} forward, {alive} alive"
                                    ))),
                            )
                            .child(NavStack::new(&self.stack).flex_1().min_h_0()),
                    ),
                ),
        )
        .into_any_element()])
    }
}
