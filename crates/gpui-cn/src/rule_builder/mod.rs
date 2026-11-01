//! A rule builder: a tree of conditions joined by `And` and `Or`, with
//! nested groups, a field, an operator, and a value per condition, and
//! drag to reorder.
//!
//! [`RuleBuilderState`] owns the rules over a set of [`RuleField`]s, and
//! [`RuleBuilder`] renders them. The same pair serves a list of actions:
//! fields with no operators, in a state kept flat with
//! [`RuleBuilderState::with_max_depth`].

mod model;
mod state;

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    assets::IconName,
    base::{StyledExt as _, TestSupportExt as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
};

pub use model::{
    Combinator, Condition, Rule, RuleEditor, RuleField, RuleGroup, RuleOperator, RuleValue,
};
pub use state::{RuleBuilderEvent, RuleBuilderState, RuleId};

use state::{NodeKind, ValueEditor};

use crate::{
    ActiveTheme as _, Button, ButtonSize, Icon, Input, Select, Tag, TagVariant, ThemeTokens,
};

/// Everything the render needs from the theme, read in one borrow.
#[derive(Clone)]
struct Look {
    row: Hsla,
    card: Hsla,
    border: Hsla,
    drop: Hsla,
    muted: Hsla,
    foreground: Hsla,
    and: (Hsla, Hsla),
    or: (Hsla, Hsla),
    radius: Pixels,
    row_height: Pixels,
    rail: Pixels,
    icon: Pixels,
    pill: Pixels,
    text: Pixels,
    small_text: Pixels,
    mono: SharedString,
}

impl Look {
    fn read(theme: &ThemeTokens) -> Self {
        Self {
            row: theme.muted(),
            card: theme.background(),
            border: theme.border(),
            drop: theme.ring(),
            muted: theme.muted_foreground(),
            foreground: theme.foreground(),
            and: (theme.info, theme.info_tint),
            or: (theme.warning, theme.warning_tint),
            radius: theme.radius_lg(),
            row_height: theme.metrics.rule_row,
            rail: theme.metrics.rule_rail,
            icon: theme.metrics.tag_icon,
            pill: theme.metrics.tag_height,
            text: theme.text_control.size,
            small_text: theme.base.typography.xs.size,
            mono: theme.mono_font_family().clone(),
        }
    }

    fn combinator(&self, combinator: Combinator) -> (Hsla, Hsla) {
        match combinator {
            Combinator::And => self.and,
            Combinator::Or => self.or,
        }
    }
}

/// The value a rule carries while it is dragged by its grip.
#[derive(Clone)]
struct DraggedRule {
    state: gpui_kit::EntityId,
    id: RuleId,
    label: SharedString,
}

/// What follows the pointer while a rule is dragged: its field's label on
/// a menu surface.
struct DragPreview {
    label: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .h(theme.metrics.control_md)
            .px_3()
            .flex()
            .items_center()
            .rounded(theme.radius_lg())
            .bg(theme.popover)
            .border_1()
            .border_color(theme.popover_border)
            .text_color(theme.popover_foreground)
            .text_size(theme.text_control.size)
            .shadow_md()
            .child(self.label.clone())
    }
}

/// A rule builder over a [`RuleBuilderState`]: the root group's rules, a
/// row per condition and a card per nested group, with the controls to
/// add rules and groups.
///
/// Each row has a grip to drag it before another rule or to the end of a
/// group, a field menu, the field's connector and operator, a value
/// editor, and a remove control. A group with two or more rules shows a
/// bracket in its rail with the combinator on it; a click on the
/// combinator switches it between `And` and `Or`.
///
/// ```no_run
/// use gpui_cn::{RuleBuilder, RuleBuilderState};
/// use gpui_kit::{Entity, IntoElement};
///
/// fn rules(state: &Entity<RuleBuilderState>) -> impl IntoElement {
///     RuleBuilder::new("rules", state)
/// }
/// ```
///
/// The id must be stable and unique under the nearest stateful ancestor.
/// A rule's elements derive their ids from it; see
/// [`RuleBuilder::rule_element_id`].
#[derive(IntoElement)]
pub struct RuleBuilder {
    id: ElementId,
    state: Entity<RuleBuilderState>,
    style: StyleRefinement,
    add_rule_label: SharedString,
    add_group_label: SharedString,
    add_nested_label: SharedString,
    group_label: SharedString,
}

impl RuleBuilder {
    /// A builder over `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<RuleBuilderState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            style: StyleRefinement::default(),
            add_rule_label: "Add rule".into(),
            add_group_label: "Add group".into(),
            add_nested_label: "Add condition".into(),
            group_label: "Group".into(),
        }
    }

    /// The label of the control that adds a rule to the root group. The
    /// default is "Add rule"; a list of actions says "Add action".
    pub fn add_rule_label(mut self, label: impl Into<SharedString>) -> Self {
        self.add_rule_label = label.into();
        self
    }

    /// The label of the control that adds a nested group. The default is
    /// "Add group".
    pub fn add_group_label(mut self, label: impl Into<SharedString>) -> Self {
        self.add_group_label = label.into();
        self
    }

    /// The label of the control that adds a condition to a nested group.
    /// The default is "Add condition".
    pub fn add_nested_label(mut self, label: impl Into<SharedString>) -> Self {
        self.add_nested_label = label.into();
        self
    }

    /// The title of a nested group's card. The default is "Group".
    pub fn group_label(mut self, label: impl Into<SharedString>) -> Self {
        self.group_label = label.into();
        self
    }

    /// The id of the element that shows `rule` in the builder with id
    /// `builder`: a condition's row or a group's card. Its parts are named
    /// children of it: `grip`, `field`, `operator`, `value`, and `remove`
    /// on a row, and `combinator`, `add-condition`, `add-group`, and
    /// `remove` on a group. The root group's parts hang off the id of the
    /// root.
    pub fn rule_element_id(builder: impl Into<ElementId>, rule: RuleId) -> ElementId {
        ElementId::NamedChild(builder.into().into(), format!("rule-{}", rule.get()).into())
    }
}

impl Styled for RuleBuilder {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The id of a named part of a rule's element.
fn part(rule: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(rule.clone().into(), name.into())
}

impl RuleBuilder {
    fn group(&self, look: &Look, id: RuleId, cx: &App) -> AnyElement {
        let state = self.state.read(cx);
        let Some(node) = state.node(id) else {
            return div().into_any_element();
        };
        let NodeKind::Group {
            combinator,
            children,
        } = &node.kind
        else {
            return div().into_any_element();
        };
        let element_id = Self::rule_element_id(self.id.clone(), id);
        let root = id == state.root();
        let rows: Vec<AnyElement> = children
            .iter()
            .map(|child| match &child.kind {
                NodeKind::Condition { .. } => self.condition(look, child.id, cx),
                NodeKind::Group { .. } => self.group(look, child.id, cx),
            })
            .collect();
        let count = rows.len();
        let body = h_flex()
            .items_stretch()
            .child(self.rail(look, &element_id, id, *combinator, count))
            .child(v_flex().flex_1().min_w_0().gap_2().children(rows));
        let footer = self.footer(look, &element_id, id, root, state.can_nest(id));

        if root {
            return v_flex()
                .id(element_id)
                .test_support()
                .gap_2()
                .child(body)
                .child(footer)
                .into_any_element();
        }

        let label = self.label(child_count_label(count), look.muted, look.small_text);
        let header = h_flex()
            .gap_2()
            .items_center()
            .px_2()
            .child(
                div()
                    .text_size(look.text)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(look.foreground)
                    .child(self.group_label.clone()),
            )
            .child(label)
            .child(div().flex_1())
            .child(self.remove_button(&element_id, id));
        let state_id = self.state.entity_id();
        let target = id;
        let entity = self.state.clone();
        v_flex()
            .id(element_id)
            .test_support()
            .gap_2()
            .p_2()
            .rounded(look.radius)
            .border_1()
            .border_color(look.border)
            .bg(look.card)
            .child(header)
            .child(body)
            .child(footer)
            .drag_over::<DraggedRule>({
                let drop = look.drop;
                move |style, dragged, _, _| {
                    if dragged.state == state_id && dragged.id != target {
                        style.border_color(drop)
                    } else {
                        style
                    }
                }
            })
            .on_drop(move |dragged: &DraggedRule, _, cx| {
                if dragged.state == state_id {
                    entity.update(cx, |state, cx| {
                        state.move_before(dragged.id, target, cx);
                    });
                    cx.stop_propagation();
                }
            })
            .into_any_element()
    }

    /// The gutter left of a group's rows, with the bracket and the
    /// combinator once the group has two rules.
    fn rail(
        &self,
        look: &Look,
        group_id: &ElementId,
        group: RuleId,
        combinator: Combinator,
        count: usize,
    ) -> AnyElement {
        let rail = div().relative().flex_none().w(look.rail);
        if count < 2 {
            return rail.into_any_element();
        }
        let inset = look.row_height / 2.;
        let (ink, tint) = look.combinator(combinator);
        let state = self.state.clone();
        let pill = Button::new(part(group_id, "combinator"))
            .ghost()
            .size(ButtonSize::Xs)
            .label(combinator.label())
            .accessibility_label(format!(
                "Rules combine with {}; switch to {}",
                combinator.label(),
                combinator.toggled().label()
            ))
            .on_click(move |_, _, cx| {
                state.update(cx, |state, cx| state.toggle_combinator(group, cx));
            })
            .h(look.pill)
            .px_2()
            .rounded_full()
            .bg(tint)
            .text_color(ink)
            .text_size(look.small_text);
        rail.child(
            div()
                .absolute()
                .top(inset)
                .bottom(inset)
                .left(look.rail / 2.)
                .right_0()
                .border_l_1()
                .border_t_1()
                .border_b_1()
                .rounded_l(look.radius)
                .border_color(look.border),
        )
        .child(div().absolute().inset_0().flex().items_center().child(pill))
        .into_any_element()
    }

    /// The add controls under a group's rows, which also take a rule
    /// dropped at the end of the group.
    fn footer(
        &self,
        look: &Look,
        group_id: &ElementId,
        group: RuleId,
        root: bool,
        can_nest: bool,
    ) -> AnyElement {
        let add_condition = {
            let state = self.state.clone();
            let label = if root {
                self.add_rule_label.clone()
            } else {
                self.add_nested_label.clone()
            };
            Button::new(part(group_id, "add-condition"))
                .outline()
                .size(ButtonSize::Sm)
                .icon(IconName::Plus)
                .label(label)
                .on_click(move |_, window, cx| {
                    state.update(cx, |state, cx| {
                        state.add_condition(group, window, cx);
                    });
                })
        };
        let add_group = (root && can_nest).then(|| {
            let state = self.state.clone();
            Button::new(part(group_id, "add-group"))
                .outline()
                .size(ButtonSize::Sm)
                .icon(Icon::from(IconName::FolderPlus))
                .label(self.add_group_label.clone())
                .on_click(move |_, window, cx| {
                    state.update(cx, |state, cx| {
                        state.add_group(group, window, cx);
                    });
                })
        });
        let state_id = self.state.entity_id();
        let entity = self.state.clone();
        h_flex()
            .id(part(group_id, "footer"))
            .test_support()
            .gap_2()
            .pl(look.rail)
            .rounded(look.radius)
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .child(add_condition)
            .children(add_group)
            .drag_over::<DraggedRule>({
                let drop = look.drop;
                move |style, dragged, _, _| {
                    if dragged.state == state_id {
                        style.border_color(drop)
                    } else {
                        style
                    }
                }
            })
            .on_drop(move |dragged: &DraggedRule, _, cx| {
                if dragged.state == state_id {
                    entity.update(cx, |state, cx| {
                        state.move_rule(dragged.id, group, usize::MAX, cx);
                    });
                    cx.stop_propagation();
                }
            })
            .into_any_element()
    }

    fn condition(&self, look: &Look, id: RuleId, cx: &App) -> AnyElement {
        let state = self.state.read(cx);
        let (Some(condition), Some(editors)) = (state.condition(id), state.editors(id)) else {
            return div().into_any_element();
        };
        let Some(field) = state.field(condition.field()) else {
            return div().into_any_element();
        };
        let row_id = Self::rule_element_id(self.id.clone(), id);
        let state_id = self.state.entity_id();
        let dragged = DraggedRule {
            state: state_id,
            id,
            label: field.label().clone(),
        };
        let grip = div()
            .id(part(&row_id, "grip"))
            .test_support()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size_5()
            .cursor_grab()
            .text_color(look.muted)
            .child(Icon::from(IconName::GripVertical).size(look.icon))
            .on_drag(dragged, |dragged, _, _, cx| {
                cx.new(|_| DragPreview {
                    label: dragged.label.clone(),
                })
            });

        let connector = field
            .connector_text()
            .map(|text| self.label(text.clone(), look.muted, look.text));
        let operator: Option<AnyElement> = match (&editors.operator, field.operators()) {
            (Some(select), _) => Some(
                Select::new(part(&row_id, "operator"), select)
                    .accessibility_label("Operator")
                    .into_any_element(),
            ),
            (None, [only]) => Some(
                self.label(only.label().clone(), look.muted, look.text)
                    .into_any_element(),
            ),
            (None, _) => None,
        };
        let editor = field.editor_for(condition.operator().map(|op| op.as_ref()));
        let value: Option<AnyElement> = match (&editors.value, editor) {
            (ValueEditor::None, _) => None,
            (ValueEditor::Text(input), _) => Some(
                Input::new(input)
                    .id(part(&row_id, "value"))
                    .accessibility_label(field.label().clone())
                    .w_40()
                    .into_any_element(),
            ),
            (ValueEditor::Choice(select), _) => Some(
                Select::new(part(&row_id, "value"), select)
                    .accessibility_label(field.label().clone())
                    .into_any_element(),
            ),
            (ValueEditor::List(input), RuleEditor::List { variant, code, .. }) => {
                let values = match condition.value() {
                    RuleValue::List(values) => values.clone(),
                    _ => Vec::new(),
                };
                Some(self.list(look, &row_id, id, &values, *variant, *code, input))
            }
            (ValueEditor::List(_), _) => None,
        };

        let entity = self.state.clone();
        h_flex()
            .id(row_id.clone())
            .test_support()
            .gap_2()
            .min_h(look.row_height)
            .px_2()
            .py_1()
            .flex_wrap()
            .rounded(look.radius)
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .bg(look.row)
            .child(grip)
            .child(Select::new(part(&row_id, "field"), &editors.field).accessibility_label("Field"))
            .children(connector)
            .children(operator)
            .children(value)
            .child(div().flex_1())
            .child(self.remove_button(&row_id, id))
            .drag_over::<DraggedRule>({
                let drop = look.drop;
                move |style, dragged, _, _| {
                    if dragged.state == state_id && dragged.id != id {
                        style.border_color(drop)
                    } else {
                        style
                    }
                }
            })
            .on_drop(move |dragged: &DraggedRule, _, cx| {
                if dragged.state == state_id {
                    entity.update(cx, |state, cx| {
                        state.move_before(dragged.id, id, cx);
                    });
                    cx.stop_propagation();
                }
            })
            .into_any_element()
    }

    /// A list value: a chip per value with a remove control, and a field
    /// to type the next one in.
    #[allow(clippy::too_many_arguments)]
    fn list(
        &self,
        look: &Look,
        row_id: &ElementId,
        id: RuleId,
        values: &[SharedString],
        variant: TagVariant,
        code: bool,
        input: &Entity<crate::InputState>,
    ) -> AnyElement {
        let chips = values.iter().map(|value| {
            let chip_id = ElementId::NamedChild(part(row_id, "value").into(), value.clone());
            let state = self.state.clone();
            let text = value.clone();
            Tag::new(chip_id.clone())
                .variant(variant)
                .when(code, |tag| tag.font_family(look.mono.clone()))
                .pr_0p5()
                .child(value.clone())
                .child(
                    Button::new(part(&chip_id, "remove"))
                        .ghost()
                        .size(ButtonSize::Xs)
                        .icon(IconName::Close)
                        .accessibility_label(format!("Remove {value}"))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| state.remove_list_value(id, &text, cx));
                        })
                        .size_4()
                        .text_color(look.muted),
                )
        });
        h_flex()
            .id(part(row_id, "value"))
            .test_support()
            .gap_1()
            .flex_wrap()
            .children(chips)
            .child(
                Input::new(input)
                    .id(part(row_id, "add-value"))
                    .accessibility_label("Add value")
                    .w_32(),
            )
            .into_any_element()
    }

    fn remove_button(&self, element_id: &ElementId, id: RuleId) -> Button {
        let state = self.state.clone();
        Button::new(part(element_id, "remove"))
            .ghost()
            .size(ButtonSize::Sm)
            .icon(IconName::Close)
            .accessibility_label("Remove")
            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.remove(id, cx)))
    }

    fn label(&self, text: SharedString, color: Hsla, size: Pixels) -> gpui_kit::Div {
        div()
            .flex_none()
            .whitespace_nowrap()
            .text_size(size)
            .text_color(color)
            .child(text)
    }
}

/// "1 condition", "2 conditions".
fn child_count_label(count: usize) -> SharedString {
    crate::plural::counted(count, "condition").into()
}

impl RenderOnce for RuleBuilder {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = Look::read(cx.theme());
        let root = self.state.read(cx).root();
        let style = self.style.clone();
        div()
            .w_full()
            .child(self.group(&look, root, cx))
            .refine_style(&style)
    }
}
