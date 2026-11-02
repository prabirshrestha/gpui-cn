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

use std::rc::Rc;

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
    Combinator, Condition, Rule, RuleEditor, RuleEditorBuilder, RuleField, RuleGroup, RuleOperator,
    RuleValue,
};
pub use state::{RuleBuilderEvent, RuleBuilderState, RuleId, RuleValueSlot};

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
    pill: Pixels,
    grip: Pixels,
    touch: bool,
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
            // A tag's height on a pointer; the touch hit target on touch.
            pill: if theme.touch {
                theme.metrics.control_md
            } else {
                theme.metrics.tag_height
            },
            grip: if theme.touch {
                theme.metrics.control_md
            } else {
                theme.metrics.control_xs
            },
            touch: theme.touch,
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

/// The key context of a rule's grip, where Alt+Up and Alt+Down move the
/// rule.
const GRIP_CONTEXT: &str = "GpuiCnRuleGrip";

gpui_kit::actions!(
    gpui_cn_rule_builder,
    [
        /// Moves the rule whose grip has focus one place up in its group.
        MoveRuleUp,
        /// Moves the rule whose grip has focus one place down in its group.
        MoveRuleDown
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("alt-up", MoveRuleUp, Some(GRIP_CONTEXT)),
        gpui_kit::KeyBinding::new("alt-down", MoveRuleDown, Some(GRIP_CONTEXT)),
    ]);
}

/// What a render hook on [`RuleBuilder`] is told about the part it draws:
/// the state, the rule, and the element id the part takes, so tests and
/// the builder's own drag and drop find it.
///
/// A replacement reads the rule through [`RulePart::state`] and changes it
/// through the state's mutations, such as
/// [`RuleBuilderState::set_field`] or
/// [`RuleBuilderState::toggle_combinator`].
#[derive(Clone)]
pub struct RulePart {
    state: Entity<RuleBuilderState>,
    rule: RuleId,
    element_id: ElementId,
}

impl RulePart {
    /// The state the builder edits.
    pub fn state(&self) -> &Entity<RuleBuilderState> {
        &self.state
    }

    /// The rule the part belongs to: a condition, or a group for the
    /// combinator, the group header, and the add controls.
    pub fn rule(&self) -> RuleId {
        self.rule
    }

    /// The id the default part uses, for the replacement to take.
    pub fn element_id(&self) -> &ElementId {
        &self.element_id
    }
}

/// A render hook: draws a part of the builder in place of the default, or
/// returns `None` to keep the default for that rule.
pub type RulePartRenderer = Rc<dyn Fn(&RulePart, &mut Window, &mut App) -> Option<AnyElement>>;

/// The render hooks of a [`RuleBuilder`], one per replaceable part.
#[derive(Clone, Default)]
struct Hooks {
    drag_handle: Option<RulePartRenderer>,
    field: Option<RulePartRenderer>,
    operator: Option<RulePartRenderer>,
    value: Option<RulePartRenderer>,
    row_end: Option<RulePartRenderer>,
    remove: Option<RulePartRenderer>,
    combinator: Option<RulePartRenderer>,
    group_header: Option<RulePartRenderer>,
    add_controls: Option<RulePartRenderer>,
}

fn renderer<E: IntoElement>(
    render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
) -> Option<RulePartRenderer> {
    Some(Rc::new(move |part, window, cx| {
        render(part, window, cx).map(IntoElement::into_any_element)
    }))
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
/// Every part can be replaced. A field or an operator can name its own
/// value editor, including the application's own view, through
/// [`RuleEditor::custom`]. The `render_*` hooks replace a part of every
/// row or group, or of the rules they choose: the drag handle, the field
/// and operator menus, the value, the remove control, the combinator, a
/// nested group's header, and the add controls, and
/// [`RuleBuilder::render_row_end`] adds controls at the end of a row. The
/// labels have their own builders.
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
    hooks: Hooks,
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
            hooks: Hooks::default(),
        }
    }

    /// Draws the inside of a rule's drag handle, in place of the grip
    /// button. The handle around it keeps the `grip` id, starts the drag,
    /// and moves the rule on Alt+Up and Alt+Down while focus is inside it;
    /// the replacement takes the `grip-content` id it is given.
    pub fn render_drag_handle<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.drag_handle = renderer(render);
        self
    }

    /// Draws a row's field picker, in place of the field menu.
    pub fn render_field<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.field = renderer(render);
        self
    }

    /// Draws a row's operator, in place of the operator menu or label.
    pub fn render_operator<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.operator = renderer(render);
        self
    }

    /// Draws a row's value, in place of its editor, for every field. A
    /// single field's own editor is better named with
    /// [`RuleEditor::custom`], which keeps its view across frames.
    pub fn render_value<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.value = renderer(render);
        self
    }

    /// Adds controls at the end of a row, before the remove control, such
    /// as a duplicate or a lock button.
    pub fn render_row_end<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.row_end = renderer(render);
        self
    }

    /// Draws the remove control of a row or a nested group.
    pub fn render_remove<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.remove = renderer(render);
        self
    }

    /// Draws a group's combinator, in place of the pill on its bracket.
    pub fn render_combinator<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.combinator = renderer(render);
        self
    }

    /// Draws a nested group's header, in place of its title, count, and
    /// remove control.
    pub fn render_group_header<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.group_header = renderer(render);
        self
    }

    /// Draws a group's add controls, in place of the add rule and add
    /// group buttons. The row they sit in still takes a rule dropped at
    /// the end of the group.
    pub fn render_add_controls<E: IntoElement>(
        mut self,
        render: impl Fn(&RulePart, &mut Window, &mut App) -> Option<E> + 'static,
    ) -> Self {
        self.hooks.add_controls = renderer(render);
        self
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
    /// children of it. A row has `grip` (with `grip-button` or
    /// `grip-content` inside), `field`, `operator`, `value`, `add-value`
    /// for a list, `end` for [`RuleBuilder::render_row_end`], and
    /// `remove`; a list's chips are named children of `value` by their
    /// text, each with its own `remove`. A group has `header`,
    /// `header-drop`, `grip`, `remove`, `combinator`, `footer`,
    /// `add-controls`, `add-condition`, and `add-group`. The root group's
    /// parts hang off the id of the root.
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
    /// Runs a hook for a part, if one is set.
    fn hook(
        &self,
        hook: &Option<RulePartRenderer>,
        rule: RuleId,
        element_id: ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let hook = hook.as_ref()?;
        let part = RulePart {
            state: self.state.clone(),
            rule,
            element_id,
        };
        hook(&part, window, cx)
    }

    fn group(&self, look: &Look, id: RuleId, window: &mut Window, cx: &mut App) -> AnyElement {
        let (combinator, children, root, can_nest) = {
            let state = self.state.read(cx);
            let Some(NodeKind::Group {
                combinator,
                children,
            }) = state.node(id).map(|node| &node.kind)
            else {
                return div().into_any_element();
            };
            let children: Vec<(RuleId, bool)> = children
                .iter()
                .map(|child| (child.id, matches!(child.kind, NodeKind::Group { .. })))
                .collect();
            (
                *combinator,
                children,
                id == state.root(),
                state.can_nest(id),
            )
        };
        let element_id = Self::rule_element_id(self.id.clone(), id);
        let rows: Vec<AnyElement> = children
            .iter()
            .map(|(child, group)| {
                if *group {
                    self.group(look, *child, window, cx)
                } else {
                    self.condition(look, *child, window, cx)
                }
            })
            .collect();
        let count = rows.len();
        let body = h_flex()
            .items_stretch()
            .child(self.rail(look, &element_id, id, combinator, count, window, cx))
            .child(v_flex().flex_1().min_w_0().gap_2().children(rows));
        let footer = self.footer(look, &element_id, id, root, can_nest, window, cx);

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
        let custom_header = self.hook(
            &self.hooks.group_header,
            id,
            part(&element_id, "header"),
            window,
            cx,
        );
        let header = match custom_header {
            Some(header) => header,
            None => {
                let remove = self.remove(&element_id, id, window, cx);
                let dragged = DraggedRule {
                    state: self.state.entity_id(),
                    id,
                    label: self.group_label.clone(),
                };
                let grip = self.grip(look, &element_id, dragged, window, cx);
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(grip)
                    .child(
                        div()
                            .text_size(look.text)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(look.foreground)
                            .child(self.group_label.clone()),
                    )
                    .child(label)
                    .child(div().flex_1())
                    .child(remove)
                    .into_any_element()
            }
        };
        let state_id = self.state.entity_id();
        let target = id;
        let entity = self.state.clone();
        // The header, not the card, takes a drop before the group: the
        // card holds the rows, which take their own drops.
        let header = div()
            .id(part(&element_id, "header-drop"))
            .rounded(look.radius)
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .child(header)
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
            });
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
            .into_any_element()
    }

    /// The gutter left of a group's rows, with the bracket and the
    /// combinator once the group has two rules.
    #[allow(clippy::too_many_arguments)]
    fn rail(
        &self,
        look: &Look,
        group_id: &ElementId,
        group: RuleId,
        combinator: Combinator,
        count: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let rail = div().relative().flex_none().w(look.rail);
        if count < 2 {
            return rail.into_any_element();
        }
        let inset = look.row_height / 2.;
        let (ink, tint) = look.combinator(combinator);
        let state = self.state.clone();
        let custom = self.hook(
            &self.hooks.combinator,
            group,
            part(group_id, "combinator"),
            window,
            cx,
        );
        let pill = custom.unwrap_or_else(|| {
            Button::new(part(group_id, "combinator"))
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
                .text_size(look.small_text)
                .into_any_element()
        });
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
    #[allow(clippy::too_many_arguments)]
    fn footer(
        &self,
        look: &Look,
        group_id: &ElementId,
        group: RuleId,
        root: bool,
        can_nest: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let custom = self.hook(
            &self.hooks.add_controls,
            group,
            part(group_id, "add-controls"),
            window,
            cx,
        );
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
            .map(|this| match custom {
                Some(custom) => this.child(custom),
                None => this.child(add_condition).children(add_group),
            })
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

    fn condition(&self, look: &Look, id: RuleId, window: &mut Window, cx: &mut App) -> AnyElement {
        let (condition, field, field_select, operator_select, value_editor) = {
            let state = self.state.read(cx);
            let (Some(condition), Some(editors)) = (state.condition(id), state.editors(id)) else {
                return div().into_any_element();
            };
            let Some(field) = state.field(condition.field()) else {
                return div().into_any_element();
            };
            (
                condition.clone(),
                field.clone(),
                editors.field.clone(),
                editors.operator.clone(),
                editors.value.clone(),
            )
        };
        let row_id = Self::rule_element_id(self.id.clone(), id);
        let state_id = self.state.entity_id();
        let dragged = DraggedRule {
            state: state_id,
            id,
            label: field.label().clone(),
        };
        let grip = self.grip(look, &row_id, dragged, window, cx);

        let connector = field
            .connector_text()
            .map(|text| self.label(text.clone(), look.muted, look.text));
        let custom_operator = self.hook(
            &self.hooks.operator,
            id,
            part(&row_id, "operator"),
            window,
            cx,
        );
        let operator: Option<AnyElement> = match (&operator_select, field.operators()) {
            _ if custom_operator.is_some() => custom_operator,
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
        let custom_value = self.hook(&self.hooks.value, id, part(&row_id, "value"), window, cx);
        let value: Option<AnyElement> = match (&value_editor, editor) {
            _ if custom_value.is_some() => custom_value,
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
            (ValueEditor::Custom(view), _) => Some(
                div()
                    .id(part(&row_id, "value"))
                    .test_support()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .child(view.clone())
                    .into_any_element(),
            ),
        };

        let field_part = self
            .hook(&self.hooks.field, id, part(&row_id, "field"), window, cx)
            .unwrap_or_else(|| {
                Select::new(part(&row_id, "field"), &field_select)
                    .accessibility_label("Field")
                    .into_any_element()
            });
        let row_end = self.hook(&self.hooks.row_end, id, part(&row_id, "end"), window, cx);
        let remove = self.remove(&row_id, id, window, cx);
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
            .child(field_part)
            .children(connector)
            .children(operator)
            .children(value)
            .child(div().flex_1())
            .children(row_end)
            .child(remove)
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

    /// A rule's grip: it starts a drag, and its button takes focus, where
    /// Alt+Up and Alt+Down move the rule in its group.
    fn grip(
        &self,
        look: &Look,
        element_id: &ElementId,
        dragged: DraggedRule,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = dragged.id;
        let custom = self.hook(
            &self.hooks.drag_handle,
            id,
            part(element_id, "grip-content"),
            window,
            cx,
        );
        let content = custom.unwrap_or_else(|| {
            Button::new(part(element_id, "grip-button"))
                .ghost()
                .size(ButtonSize::Xs)
                .icon(Icon::from(IconName::GripVertical))
                .accessibility_label("Move rule; Alt+Up or Alt+Down")
                .w(look.grip)
                .h(look.grip)
                .text_color(look.muted)
                .into_any_element()
        });
        let (up, down) = (self.state.clone(), self.state.clone());
        div()
            .id(part(element_id, "grip"))
            .test_support()
            .key_context(GRIP_CONTEXT)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(look.grip)
            .cursor_grab()
            .child(content)
            .on_action(move |_: &MoveRuleUp, _, cx| {
                up.update(cx, |state, cx| state.move_by(id, -1, cx));
            })
            .on_action(move |_: &MoveRuleDown, _, cx| {
                down.update(cx, |state, cx| state.move_by(id, 1, cx));
            })
            .on_drag(dragged, |dragged, _, _, cx| {
                cx.new(|_| DragPreview {
                    label: dragged.label.clone(),
                })
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
                        .when(!look.touch, |button| button.size_4())
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

    fn remove(
        &self,
        element_id: &ElementId,
        id: RuleId,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if let Some(custom) = self.hook(
            &self.hooks.remove,
            id,
            part(element_id, "remove"),
            window,
            cx,
        ) {
            return custom;
        }
        let state = self.state.clone();
        Button::new(part(element_id, "remove"))
            .ghost()
            .size(ButtonSize::Sm)
            .icon(IconName::Close)
            .accessibility_label("Remove")
            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.remove(id, cx)))
            .into_any_element()
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = Look::read(cx.theme());
        let root = self.state.read(cx).root();
        let style = self.style.clone();
        div()
            .w_full()
            .child(self.group(&look, root, window, cx))
            .refine_style(&style)
    }
}
