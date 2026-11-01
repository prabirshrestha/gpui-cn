//! UI integration tests for `RuleBuilder`.

use std::{cell::RefCell, rc::Rc};

use gpui_cn::{
    Combinator, Condition, ReduceMotion, Rule, RuleBuilder, RuleBuilderEvent, RuleBuilderState,
    RuleEditor, RuleField, RuleGroup, RuleId, RuleOperator, RuleValue, SelectItem, TagVariant,
    Theme,
};
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TestAppContext, Window, WindowHandle, base::Root, div, px, size,
    test::TestWindowExt as _,
};

struct Harness {
    state: Entity<RuleBuilderState>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(RuleBuilder::new("rules", &self.state))
    }
}

struct Setup {
    handle: WindowHandle<Root>,
    state: Entity<RuleBuilderState>,
    events: Rc<RefCell<Vec<RuleBuilderEvent>>>,
}

fn fields() -> Vec<RuleField> {
    vec![
        RuleField::new("command", "Command")
            .connector("is")
            .operator(RuleOperator::new("any", "Any of"))
            .operator(RuleOperator::new("none", "None of"))
            .editor(RuleEditor::code_list("Add command", TagVariant::Success)),
        RuleField::new("branch", "Branch")
            .operator(RuleOperator::new("eq", "Equal to"))
            .operator(RuleOperator::new("ne", "Not equal to"))
            .operator(RuleOperator::new("empty", "Is empty").unary())
            .editor(RuleEditor::text("main")),
        RuleField::new("agent", "Agent")
            .connector("is")
            .operator(RuleOperator::new("eq", "Equal to"))
            .editor(RuleEditor::choice([
                SelectItem::new(SharedString::from("codex"), "Codex"),
                SelectItem::new(SharedString::from("claude"), "Claude"),
            ])),
    ]
}

fn query() -> RuleGroup {
    RuleGroup::and([
        Rule::from(Condition::new("command").with_list(["git push", "rm -rf"])),
        Rule::from(RuleGroup::or([
            Condition::new("branch")
                .with_operator("eq")
                .with_value(RuleValue::Text("main".into())),
            Condition::new("agent"),
        ])),
    ])
}

fn setup(cx: &mut TestAppContext, query: RuleGroup) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut state = None;
    let handle = cx.open_window(size(px(900.), px(800.)), |window, cx| {
        let rules =
            cx.new(|cx| RuleBuilderState::new(fields(), window, cx).with_query(query, window, cx));
        let recorded = events.clone();
        cx.subscribe(&rules, move |_, _, event: &RuleBuilderEvent, _| {
            recorded.borrow_mut().push(event.clone());
        })
        .detach();
        state = Some(rules.clone());
        let harness = cx.new(|cx| {
            cx.observe(&rules, |_, _, cx| cx.notify()).detach();
            Harness { state: rules }
        });
        Root::new(harness, window, cx)
    });
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    Setup {
        handle,
        state: state.unwrap(),
        events,
    }
}

fn rule(id: RuleId) -> ElementId {
    RuleBuilder::rule_element_id("rules", id)
}

fn part(id: RuleId, name: &'static str) -> ElementId {
    ElementId::NamedChild(rule(id).into(), name.into())
}

/// The root's rules, then the nested group's.
fn ids(setup: &Setup, cx: &mut TestAppContext) -> (Vec<RuleId>, Vec<RuleId>) {
    setup.state.read_with(cx, |state, _| {
        let top = state.children(state.root());
        let nested = top
            .iter()
            .find(|id| state.combinator(**id).is_some())
            .map(|id| state.children(*id))
            .unwrap_or_default();
        (top, nested)
    })
}

#[gpui_kit::test]
fn a_query_renders_rows_and_a_group_card_and_round_trips(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, nested) = ids(&setup, cx);
    assert_eq!(top.len(), 2);
    assert_eq!(nested.len(), 2);
    let row_height = cx.update(|cx| gpui_cn::ActiveTheme::theme(cx).metrics.rule_row);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let first = window.find(rule(top[0])).bounds();
        let group = window.find(rule(top[1])).bounds();
        let inner = window.find(rule(nested[0])).bounds();
        assert!(first.size.height >= row_height);
        assert!(group.origin.y > first.origin.y, "the group comes second");
        assert!(group.contains(&inner.center()), "the card holds its rows");
        assert!(inner.origin.x > first.origin.x, "a nested row is indented");
        assert!(window.try_find(part(top[0], "grip")).is_some());
    })
    .unwrap();
    let round_trip = setup.state.read_with(cx, |state, _| state.query());
    assert_eq!(
        round_trip,
        RuleGroup::and([
            Rule::from(
                Condition::new("command")
                    .with_operator("any")
                    .with_list(["git push", "rm -rf"])
            ),
            Rule::from(RuleGroup::or([
                Condition::new("branch")
                    .with_operator("eq")
                    .with_value(RuleValue::Text("main".into())),
                Condition::new("agent")
                    .with_operator("eq")
                    .with_value(RuleValue::Choice("codex".into())),
            ])),
        ])
    );
}

#[gpui_kit::test]
fn the_combinator_pill_switches_and_and_or(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let root = setup.state.read_with(cx, |state, _| state.root());
    let (top, _) = ids(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(part(root, "combinator"), cx);
        window.render_frame(cx);
        window.click(part(top[1], "combinator"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    setup.state.read_with(cx, |state, _| {
        assert_eq!(state.combinator(root), Some(Combinator::Or));
        assert_eq!(state.combinator(top[1]), Some(Combinator::And));
    });
    assert_eq!(setup.events.borrow().len(), 2);
}

#[gpui_kit::test]
fn add_controls_add_rules_and_one_level_of_groups(cx: &mut TestAppContext) {
    let setup = setup(cx, RuleGroup::default());
    let root = setup.state.read_with(cx, |state, _| state.root());
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert!(
            window.try_find(part(root, "combinator")).is_none(),
            "no combinator under two rules"
        );
        window.click(part(root, "add-condition"), cx);
        window.render_frame(cx);
        window.click(part(root, "add-group"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let (top, nested) = ids(&setup, cx);
    assert_eq!(top.len(), 2);
    assert_eq!(nested.len(), 1, "a new group starts with one condition");
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert!(window.try_find(part(root, "combinator")).is_some());
        assert!(
            window.try_find(part(top[1], "add-group")).is_none(),
            "groups nest one level by default"
        );
        let add = window.find(part(top[1], "add-condition")).bounds();
        let group = window.find(rule(top[1])).bounds();
        assert!(group.contains(&add.center()));
        window.click(part(top[1], "add-condition"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(ids(&setup, cx).1.len(), 2);
    let added = setup
        .state
        .read_with(cx, |state, _| state.condition(top[0]).cloned().unwrap());
    assert_eq!(added.field().as_ref(), "command", "the first field");
    assert_eq!(added.operator().map(|op| op.as_ref()), Some("any"));
}

#[gpui_kit::test]
fn removing_the_last_rule_of_a_group_removes_the_group(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, nested) = ids(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(part(nested[0], "remove"), cx);
        window.render_frame(cx);
        assert!(window.try_find(rule(nested[0])).is_none());
        assert!(window.try_find(rule(top[1])).is_some());
        window.click(part(nested[1], "remove"), cx);
        window.render_frame(cx);
        assert!(
            window.try_find(rule(top[1])).is_none(),
            "the empty group goes"
        );
    })
    .unwrap();
    assert_eq!(ids(&setup, cx).0, vec![top[0]]);
}

#[gpui_kit::test]
fn dragging_a_grip_moves_a_rule_before_another_or_into_a_group(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, nested) = ids(&setup, cx);
    // Out of the group, before the first rule.
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.drag_to(part(nested[1], "grip"), rule(top[0]), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let (after, inner) = ids(&setup, cx);
    assert_eq!(after, vec![nested[1], top[0], top[1]]);
    assert_eq!(inner, vec![nested[0]]);
    // Into the group, at its end, by its footer.
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.drag_to(part(top[0], "grip"), part(top[1], "footer"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let (after, inner) = ids(&setup, cx);
    assert_eq!(after, vec![nested[1], top[1]]);
    assert_eq!(inner, vec![nested[0], top[0]]);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let group = window.find(rule(top[1])).bounds();
        assert!(group.contains(&window.find(rule(top[0])).bounds().center()));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_group_never_moves_into_itself_or_deeper_than_allowed(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, nested) = ids(&setup, cx);
    let moved = setup.state.update(cx, |state, cx| {
        [
            state.move_rule(top[1], top[1], 0, cx),
            state.move_before(top[1], nested[0], cx),
        ]
    });
    assert_eq!(moved, [false, false]);
    assert_eq!(ids(&setup, cx), (top, nested));
    assert!(setup.events.borrow().is_empty());
}

#[gpui_kit::test]
fn choosing_a_field_or_an_operator_swaps_the_value_editor(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, nested) = ids(&setup, cx);
    let field = part(top[0], "field");
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(
            ElementId::NamedChild(field.clone().into(), "trigger".into()),
            cx,
        );
        window.render_frame(cx);
        window.click(
            ElementId::NamedChild(field.clone().into(), "branch".into()),
            cx,
        );
        window.render_frame(cx);
        assert!(window.try_find(part(top[0], "operator")).is_some());
    })
    .unwrap();
    let changed = setup
        .state
        .read_with(cx, |state, _| state.condition(top[0]).cloned().unwrap());
    assert_eq!(changed.field().as_ref(), "branch");
    assert_eq!(changed.operator().map(|op| op.as_ref()), Some("eq"));
    assert_eq!(changed.value(), &RuleValue::Text("".into()));

    // A unary operator takes no value.
    let operator = part(nested[0], "operator");
    cx.update_window(setup.handle.into(), |_, window, cx| {
        assert!(window.try_find(part(nested[0], "value")).is_some());
        window.click(
            ElementId::NamedChild(operator.clone().into(), "trigger".into()),
            cx,
        );
        window.render_frame(cx);
        window.click(
            ElementId::NamedChild(operator.clone().into(), "empty".into()),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(part(nested[0], "value")).is_none());
    })
    .unwrap();
    let unary = setup
        .state
        .read_with(cx, |state, _| state.condition(nested[0]).cloned().unwrap());
    assert_eq!(unary.value(), &RuleValue::None);
}

#[gpui_kit::test]
fn a_list_value_adds_on_enter_and_removes_by_its_chip(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (top, _) = ids(&setup, cx);
    let value = part(top[0], "value");
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(part(top[0], "add-value"), cx);
        window.input("npm publish", cx);
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    // Events reach the state when the window's update ends.
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let chip = ElementId::NamedChild(value.clone().into(), "npm publish".into());
        assert!(window.try_find(chip).is_some());
        let git = ElementId::NamedChild(value.clone().into(), "git push".into());
        window.click(
            ElementId::NamedChild(git.clone().into(), "remove".into()),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let git = ElementId::NamedChild(value.clone().into(), "git push".into());
        assert!(window.try_find(git).is_none());
    })
    .unwrap();
    let list = setup.state.read_with(cx, |state, _| {
        state.condition(top[0]).unwrap().value().clone()
    });
    assert_eq!(
        list,
        RuleValue::List(vec!["rm -rf".into(), "npm publish".into()])
    );
}

#[gpui_kit::test]
fn a_text_value_follows_typing(cx: &mut TestAppContext) {
    let setup = setup(cx, query());
    let (_, nested) = ids(&setup, cx);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.click(part(nested[0], "value"), cx);
        window.input("-dev", cx);
        window.render_frame(cx);
    })
    .unwrap();
    let text = setup.state.read_with(cx, |state, _| {
        state.condition(nested[0]).unwrap().value().clone()
    });
    assert_eq!(text, RuleValue::Text("main-dev".into()));
}

#[gpui_kit::test]
fn a_flat_state_lists_actions_without_groups(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
    });
    let actions = [
        RuleField::new("notify", "Notify").editor(RuleEditor::choice([SelectItem::new(
            SharedString::from("deploys"),
            "#deploys",
        )])),
    ];
    let mut flat = None;
    let handle = cx.open_window(size(px(600.), px(400.)), |window, cx| {
        let state = cx.new(|cx| {
            RuleBuilderState::new(actions, window, cx)
                .with_max_depth(1)
                .with_query(
                    RuleGroup::and([Rule::from(RuleGroup::and([Condition::new("notify")]))]),
                    window,
                    cx,
                )
        });
        flat = Some(state.clone());
        let harness = cx.new(|_| Harness { state });
        Root::new(harness, window, cx)
    });
    let state = flat.unwrap();
    cx.run_until_parked();
    let (root, query) = state.read_with(cx, |state, _| (state.root(), state.query()));
    assert_eq!(
        query,
        RuleGroup::and([Condition::new("notify").with_value(RuleValue::Choice("deploys".into()))]),
        "a group past the depth is inlined"
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(part(root, "add-group")).is_none());
        assert!(window.try_find(part(root, "add-condition")).is_some());
    })
    .unwrap();
    assert!(!state.read_with(cx, |state, _| state.can_nest(root)));
}
