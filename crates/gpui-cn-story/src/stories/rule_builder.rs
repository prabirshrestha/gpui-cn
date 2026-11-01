use gpui_cn::{
    ActiveTheme as _, Condition, Rule, RuleBuilder, RuleBuilderState, RuleEditor, RuleField,
    RuleGroup, RuleOperator, RuleValue, Select, SelectItem, SelectState, Switch, Tag, TagVariant,
    gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Window, base::h_flex, div, prelude::FluentBuilder as _,
};

use crate::{PAGE_WIDTH, Story, note, page, section};

/// A policy for coding agents: when an agent wants to act, conditions on
/// the command, the branch, the files, and the repository decide which
/// actions run, after the rule builder post the component follows.
pub struct RuleBuilderStory {
    enabled: bool,
    trigger: Entity<SelectState<&'static str>>,
    conditions: Entity<RuleBuilderState>,
    actions: Entity<RuleBuilderState>,
    otherwise: Entity<RuleBuilderState>,
}

impl Story for RuleBuilderStory {
    fn title() -> &'static str {
        "Rule builder"
    }

    fn icon() -> IconName {
        IconName::Network
    }

    fn description() -> &'static str {
        "Builds conditions joined by And and Or, with nested groups, and the actions that follow."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| Self::new(window, cx)).into()
    }
}

fn text(key: &'static str) -> SharedString {
    key.into()
}

/// What a condition can test.
fn condition_fields() -> Vec<RuleField> {
    let set_operators = |field: RuleField| {
        field
            .operator(RuleOperator::new("any", "Any of"))
            .operator(RuleOperator::new("all", "All of"))
            .operator(RuleOperator::new("none", "None of"))
    };
    let equality = |field: RuleField| {
        field
            .operator(RuleOperator::new("eq", "Equal to"))
            .operator(RuleOperator::new("ne", "Not equal to"))
    };
    vec![
        set_operators(
            RuleField::new("command", "Command")
                .icon(IconName::SquareTerminal)
                .connector("is"),
        )
        .editor(RuleEditor::code_list("Add command", TagVariant::Success)),
        equality(RuleField::new("branch", "Branch").icon(IconName::GitBranch))
            .operator(
                RuleOperator::new("matches", "Matches")
                    .editor(RuleEditor::code_list("release/*", TagVariant::Info)),
            )
            .editor(RuleEditor::text("main")),
        set_operators(
            RuleField::new("files", "Files changed")
                .icon(IconName::FileDiff)
                .connector("include"),
        )
        .editor(RuleEditor::code_list("Add path", TagVariant::Warning)),
        equality(
            RuleField::new("repository", "Repository")
                .icon(IconName::Github)
                .connector("is"),
        )
        .editor(RuleEditor::choice([
            SelectItem::new(text("acme/payments"), "acme/payments").icon(IconName::Github),
            SelectItem::new(text("acme/web"), "acme/web").icon(IconName::Github),
            SelectItem::new(text("acme/infra"), "acme/infra").icon(IconName::Github),
        ])),
        equality(
            RuleField::new("agent", "Agent")
                .icon(IconName::Bot)
                .connector("is"),
        )
        .editor(RuleEditor::choice([
            SelectItem::new(text("codex"), "Codex"),
            SelectItem::new(text("claude"), "Claude"),
            SelectItem::new(text("any"), "Any agent"),
        ])),
    ]
}

/// What a rule can do.
fn action_fields() -> Vec<RuleField> {
    vec![
        RuleField::new("approval", "Ask for approval")
            .icon(IconName::Users)
            .connector("from")
            .editor(RuleEditor::choice([
                SelectItem::new(text("owners"), "Code owners"),
                SelectItem::new(text("leads"), "Team leads"),
                SelectItem::new(text("anyone"), "Anyone on the team"),
            ])),
        RuleField::new("notify", "Notify")
            .icon(IconName::Bell)
            .editor(RuleEditor::choice([
                SelectItem::new(text("deploys"), "#deploys"),
                SelectItem::new(text("security"), "#security"),
            ])),
        RuleField::new("expire", "Expire after")
            .icon(IconName::Clock)
            .editor(RuleEditor::choice([
                SelectItem::new(text("30m"), "30 minutes"),
                SelectItem::new(text("2h"), "2 hours"),
                SelectItem::new(text("1d"), "1 day"),
            ])),
        RuleField::new("allow", "Allow")
            .icon(IconName::ShieldCheck)
            .editor(RuleEditor::choice([
                SelectItem::new(text("silent"), "Without asking"),
                SelectItem::new(text("once"), "Once"),
            ])),
        RuleField::new("block", "Block").icon(IconName::TriangleAlert),
        RuleField::new("log", "Log to")
            .icon(IconName::ClipboardList)
            .editor(RuleEditor::choice([
                SelectItem::new(text("audit"), "Audit trail"),
                SelectItem::new(text("stdout"), "Session log"),
            ])),
    ]
}

impl RuleBuilderStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let trigger = cx.new(|cx| {
            SelectState::new(
                [
                    SelectItem::new("command", "Run a command"),
                    SelectItem::new("edit", "Edit files"),
                    SelectItem::new("api", "Call an external API"),
                    SelectItem::new("spend", "Spend over $5"),
                ],
                cx,
            )
            .with_selected(["command"])
        });
        let conditions = cx.new(|cx| {
            RuleBuilderState::new(condition_fields(), window, cx).with_query(
                RuleGroup::and([
                    Rule::from(Condition::new("command").with_list([
                        "git push",
                        "npm publish",
                        "rm -rf",
                    ])),
                    Rule::from(RuleGroup::or([
                        Condition::new("branch")
                            .with_operator("eq")
                            .with_value(RuleValue::Text("main".into())),
                        Condition::new("files").with_list(["migrations/*"]),
                    ])),
                ]),
                window,
                cx,
            )
        });
        let actions = cx.new(|cx| {
            RuleBuilderState::new(action_fields(), window, cx)
                .with_max_depth(1)
                .with_query(
                    RuleGroup::and([
                        Condition::new("approval"),
                        Condition::new("notify"),
                        Condition::new("expire"),
                    ]),
                    window,
                    cx,
                )
        });
        let otherwise = cx.new(|cx| {
            RuleBuilderState::new(action_fields(), window, cx)
                .with_max_depth(1)
                .with_query(
                    RuleGroup::and([Condition::new("allow"), Condition::new("log")]),
                    window,
                    cx,
                )
        });
        for state in [&conditions, &actions, &otherwise] {
            cx.observe(state, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&trigger, |_, _, cx| cx.notify()).detach();
        Self {
            enabled: true,
            trigger,
            conditions,
            actions,
            otherwise,
        }
    }
}

/// The query as one line of text, so the story shows what the builder
/// produces as it changes.
fn describe(group: &RuleGroup, fields: &[RuleField]) -> String {
    let joiner = format!(" {} ", group.combinator().label().to_uppercase());
    group
        .rules()
        .iter()
        .map(|rule| match rule {
            Rule::Group(inner) => format!("({})", describe(inner, fields)),
            Rule::Condition(condition) => {
                let field = fields.iter().find(|field| field.key() == condition.field());
                let name = field.map_or(condition.field().clone(), |f| f.label().clone());
                let operator = condition
                    .operator()
                    .and_then(|key| field.and_then(|f| f.find_operator(key)))
                    .map(|op| op.label().to_lowercase())
                    .unwrap_or_default();
                let value = match condition.value() {
                    RuleValue::Text(text) => format!("\"{text}\""),
                    RuleValue::Choice(key) => key.to_string(),
                    RuleValue::List(values) => format!("[{}]", values.join(", ")),
                    _ => String::new(),
                };
                [name.to_string(), operator, value]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join(&joiner)
}

impl Render for RuleBuilderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground();
        let row = theme.muted();
        let radius = theme.radius_lg();
        let row_height = theme.metrics.rule_row;
        let mono = theme.mono_font_family().clone();
        let disabled = theme.disabled_opacity;
        let heading = |title: &'static str| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(title)
        };
        let query = {
            let state = self.conditions.read(cx);
            describe(&state.query(), state.fields())
        };
        let entity = cx.entity();

        let policy = div()
            .max_w(PAGE_WIDTH)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                h_flex()
                    .items_start()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Guard risky commands"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Every agent in acme - Last matched 12 min ago"),
                            ),
                    )
                    .child(
                        Switch::new("policy-enabled")
                            .checked(self.enabled)
                            .accessibility_label("Policy enabled")
                            .on_change(move |checked, _, cx| {
                                let checked = *checked;
                                entity.update(cx, |story, cx| {
                                    story.enabled = checked;
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(heading("WHEN"))
            .child(
                h_flex()
                    .gap_2()
                    .min_h(row_height)
                    .px_2()
                    .rounded(radius)
                    .bg(row)
                    .child(div().text_sm().text_color(muted).child("Agent wants to"))
                    .child(Select::new("trigger", &self.trigger).accessibility_label("Trigger")),
            )
            .child(RuleBuilder::new("conditions", &self.conditions))
            .child(heading("THEN"))
            .child(
                h_flex().child(
                    Tag::new("when-matches")
                        .icon(IconName::Users)
                        .label("When it matches")
                        .warning(),
                ),
            )
            .child(RuleBuilder::new("actions", &self.actions).add_rule_label("Add action"))
            .child(
                h_flex().child(
                    Tag::new("otherwise")
                        .icon(IconName::Check)
                        .label("Otherwise")
                        .success(),
                ),
            )
            .child(
                RuleBuilder::new("otherwise-actions", &self.otherwise).add_rule_label("Add action"),
            )
            .when(!self.enabled, |this| this.opacity(disabled));

        page([
            section("Policy", policy).into_any_element(),
            section(
                "Query",
                div()
                    .max_w(PAGE_WIDTH)
                    .p_3()
                    .rounded(radius)
                    .bg(row)
                    .font_family(mono)
                    .text_sm()
                    .child(query),
            )
            .into_any_element(),
            note(
                "Drag a grip to move a rule before another or to the end of a group. Click And or Or to switch it. Type a value in a list and press Enter to add it.",
                cx,
            )
            .into_any_element(),
        ])
    }
}
