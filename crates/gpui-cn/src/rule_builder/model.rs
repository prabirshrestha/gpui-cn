//! The plain data of a rule tree: the fields a condition can test, their
//! operators and value editors, and the conditions and groups a query is
//! made of.

use std::rc::Rc;

use gpui_kit::{AnyView, App, SharedString, Window};

use super::state::RuleValueSlot;
use crate::{Icon, SelectItem, TagVariant};

/// Builds a custom value editor's view for one condition row: given the
/// row's current value and the slot to report new values to.
pub type RuleEditorBuilder =
    Rc<dyn Fn(&RuleValue, RuleValueSlot, &mut Window, &mut App) -> AnyView>;

/// How the rules of a group combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Combinator {
    /// Every rule must match.
    #[default]
    And,
    /// One rule is enough.
    Or,
}

impl Combinator {
    /// The other combinator.
    pub fn toggled(self) -> Self {
        match self {
            Self::And => Self::Or,
            Self::Or => Self::And,
        }
    }

    /// The word the connector pill shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::And => "And",
            Self::Or => "Or",
        }
    }
}

/// How a condition takes its value, after its field and operator.
#[derive(Clone)]
#[non_exhaustive]
pub enum RuleEditor {
    /// No value: the operator says it all, such as "is empty".
    None,
    /// One line of text.
    Text {
        /// The text the field shows while it is empty.
        placeholder: SharedString,
    },
    /// One item of a menu. The first item is the default.
    Choice(Vec<SelectItem<SharedString>>),
    /// A list of values, each a removable chip, typed one at a time and
    /// added with Enter.
    List {
        /// The text the field shows while it is empty.
        placeholder: SharedString,
        /// The color of the chips.
        variant: TagVariant,
        /// Whether the chips use the code font, for commands and paths.
        code: bool,
    },
    /// The application's own view, such as a code editor. See
    /// [`RuleEditor::custom`].
    Custom {
        /// Builds the view for a row.
        build: RuleEditorBuilder,
        /// The value a new condition starts with.
        default: RuleValue,
    },
}

impl RuleEditor {
    /// A [`RuleEditor::Text`].
    pub fn text(placeholder: impl Into<SharedString>) -> Self {
        Self::Text {
            placeholder: placeholder.into(),
        }
    }

    /// A [`RuleEditor::Choice`].
    pub fn choice(items: impl IntoIterator<Item = SelectItem<SharedString>>) -> Self {
        Self::Choice(items.into_iter().collect())
    }

    /// A [`RuleEditor::List`] of chips in the given color.
    pub fn list(placeholder: impl Into<SharedString>, variant: TagVariant) -> Self {
        Self::List {
            placeholder: placeholder.into(),
            variant,
            code: false,
        }
    }

    /// A [`RuleEditor::List`] whose chips use the code font.
    pub fn code_list(placeholder: impl Into<SharedString>, variant: TagVariant) -> Self {
        Self::List {
            placeholder: placeholder.into(),
            variant,
            code: true,
        }
    }

    /// An editor the application draws itself, such as a code editor or a
    /// date picker. `build` runs once for each row that uses the editor,
    /// when the row is created and again when its field or operator
    /// changes. It returns a view the application owns, seeded with the
    /// row's value, and the view reports each new value through the
    /// [`RuleValueSlot`]. A custom editor takes a value of any kind; a row
    /// with no value starts with `default`.
    ///
    /// ```no_run
    /// use gpui_cn::{RuleEditor, RuleValue, TextareaState};
    /// use gpui_kit::{AppContext as _, Render};
    ///
    /// # struct CodeEditor;
    /// # impl Render for CodeEditor {
    /// #     fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement { gpui_kit::div() }
    /// # }
    /// let _ = RuleEditor::custom(RuleValue::Text("".into()), |value, slot, window, cx| {
    ///     let _ = (value, slot, window);
    ///     cx.new(|_| CodeEditor).into()
    /// });
    /// ```
    pub fn custom(
        default: RuleValue,
        build: impl Fn(&RuleValue, RuleValueSlot, &mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        Self::Custom {
            build: Rc::new(build),
            default,
        }
    }

    /// The value a new condition starts with.
    pub(crate) fn default_value(&self) -> RuleValue {
        match self {
            Self::None => RuleValue::None,
            Self::Text { .. } => RuleValue::Text(SharedString::default()),
            Self::Choice(items) => items
                .iter()
                .find(|item| !item.is_disabled())
                .map(|item| RuleValue::Choice(item.value().clone()))
                .unwrap_or(RuleValue::None),
            Self::List { .. } => RuleValue::List(Vec::new()),
            Self::Custom { default, .. } => default.clone(),
        }
    }

    /// Whether `value` is the kind of value this editor edits.
    pub(crate) fn accepts(&self, value: &RuleValue) -> bool {
        match (self, value) {
            // A custom editor takes any kind, but a missing value takes
            // its default.
            (Self::Custom { default, .. }, value) => {
                *value != RuleValue::None || *default == RuleValue::None
            }
            (Self::None, RuleValue::None)
            | (Self::Text { .. }, RuleValue::Text(_))
            | (Self::List { .. }, RuleValue::List(_)) => true,
            (Self::Choice(items), RuleValue::Choice(key)) => {
                items.iter().any(|item| item.value() == key)
            }
            _ => false,
        }
    }
}

impl RuleValue {
    /// Drops blank and repeated values from a list, keeping the first of
    /// each, so every chip has its own text.
    pub(crate) fn dedupe(&mut self) {
        if let Self::List(values) = self {
            let mut seen = Vec::with_capacity(values.len());
            values.retain(|value| {
                let keep = !value.trim().is_empty() && !seen.contains(value);
                if keep {
                    seen.push(value.clone());
                }
                keep
            });
        }
    }
}

/// The value of a condition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuleValue {
    /// No value.
    #[default]
    None,
    /// The text of a [`RuleEditor::Text`].
    Text(SharedString),
    /// The key of the chosen item of a [`RuleEditor::Choice`].
    Choice(SharedString),
    /// The values of a [`RuleEditor::List`], in the order added.
    List(Vec<SharedString>),
}

/// What a condition does with its value, such as "Equal to" or "Any of".
///
/// An operator takes the value editor of its field unless it names its
/// own, and a unary one takes no value.
#[derive(Clone)]
pub struct RuleOperator {
    key: SharedString,
    label: SharedString,
    editor: Option<RuleEditor>,
}

impl RuleOperator {
    /// An operator with a stable key and a visible label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            editor: None,
        }
    }

    /// The value editor this operator uses in place of its field's.
    pub fn editor(mut self, editor: RuleEditor) -> Self {
        self.editor = Some(editor);
        self
    }

    /// Takes no value, such as "is empty".
    pub fn unary(self) -> Self {
        self.editor(RuleEditor::None)
    }

    /// The stable key.
    pub fn key(&self) -> &SharedString {
        &self.key
    }

    /// The visible label.
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// Something a condition can test, such as a branch or a command, with
/// the operators it allows and how it takes its value.
///
/// A field with one operator shows the operator as text; with more it
/// shows a menu. A field with none, such as an action in a list of
/// actions, shows only its value. The connector is a word between the
/// field and the operator, such as "is" or "from".
///
/// ```
/// use gpui_cn::{RuleEditor, RuleField, RuleOperator, TagVariant};
///
/// let _ = RuleField::new("branch", "Branch")
///     .operator(RuleOperator::new("eq", "Equal to"))
///     .operator(RuleOperator::new("ne", "Not equal to"))
///     .editor(RuleEditor::text("main"));
/// let _ = RuleField::new("files", "Files changed")
///     .connector("include")
///     .operator(RuleOperator::new("any", "Any of"))
///     .editor(RuleEditor::code_list("migrations/*", TagVariant::Warning));
/// ```
#[derive(Clone)]
pub struct RuleField {
    key: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    connector: Option<SharedString>,
    operators: Vec<RuleOperator>,
    editor: RuleEditor,
}

impl RuleField {
    /// A field with a stable key and a visible label, no operators, and no
    /// value.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            connector: None,
            operators: Vec::new(),
            editor: RuleEditor::None,
        }
    }

    /// The icon before the label, in the menu and the trigger.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The word between the field and the operator.
    pub fn connector(mut self, connector: impl Into<SharedString>) -> Self {
        self.connector = Some(connector.into());
        self
    }

    /// Adds an operator. The first is the default.
    pub fn operator(mut self, operator: RuleOperator) -> Self {
        self.operators.push(operator);
        self
    }

    /// How the value is taken, unless the operator names its own editor.
    pub fn editor(mut self, editor: RuleEditor) -> Self {
        self.editor = editor;
        self
    }

    /// The stable key.
    pub fn key(&self) -> &SharedString {
        &self.key
    }

    /// The visible label.
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// The icon, if any.
    pub fn icon_source(&self) -> Option<&Icon> {
        self.icon.as_ref()
    }

    /// The connector word, if any.
    pub fn connector_text(&self) -> Option<&SharedString> {
        self.connector.as_ref()
    }

    /// The operators, in order.
    pub fn operators(&self) -> &[RuleOperator] {
        &self.operators
    }

    /// The operator with `key`, if the field has it.
    pub fn find_operator(&self, key: &str) -> Option<&RuleOperator> {
        self.operators.iter().find(|op| op.key.as_ref() == key)
    }

    /// The editor a condition on this field uses under `operator`.
    pub fn editor_for(&self, operator: Option<&str>) -> &RuleEditor {
        operator
            .and_then(|key| self.find_operator(key))
            .and_then(|op| op.editor.as_ref())
            .unwrap_or(&self.editor)
    }
}

/// One test: a field, an operator, and a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    field: SharedString,
    operator: Option<SharedString>,
    value: RuleValue,
}

impl Condition {
    /// A condition on the field with `field` as its key, with the field's
    /// default operator and value until they are seeded.
    pub fn new(field: impl Into<SharedString>) -> Self {
        Self {
            field: field.into(),
            operator: None,
            value: RuleValue::None,
        }
    }

    /// Seeds the operator by key.
    pub fn with_operator(mut self, operator: impl Into<SharedString>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// Seeds the value.
    pub fn with_value(mut self, value: RuleValue) -> Self {
        self.value = value;
        self
    }

    /// Seeds a list value.
    pub fn with_list(self, values: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.with_value(RuleValue::List(
            values.into_iter().map(Into::into).collect(),
        ))
    }

    /// The key of the field.
    pub fn field(&self) -> &SharedString {
        &self.field
    }

    /// The key of the operator, if the field has operators.
    pub fn operator(&self) -> Option<&SharedString> {
        self.operator.as_ref()
    }

    /// The value.
    pub fn value(&self) -> &RuleValue {
        &self.value
    }

    /// Fills in the operator and the value from `field` where they are
    /// missing or do not fit it.
    pub(crate) fn normalize(&mut self, field: &RuleField) {
        let operator_fits = self
            .operator
            .as_ref()
            .is_some_and(|key| field.find_operator(key).is_some());
        if !operator_fits {
            self.operator = field.operators.first().map(|op| op.key.clone());
        }
        let editor = field.editor_for(self.operator.as_deref());
        if !editor.accepts(&self.value) {
            self.value = editor.default_value();
        }
        self.value.dedupe();
    }

    pub(crate) fn set_operator(&mut self, operator: Option<SharedString>) {
        self.operator = operator;
    }

    pub(crate) fn set_value(&mut self, value: RuleValue) {
        self.value = value;
    }

    pub(crate) fn value_mut(&mut self) -> &mut RuleValue {
        &mut self.value
    }
}

/// A set of rules joined by one combinator.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleGroup {
    combinator: Combinator,
    rules: Vec<Rule>,
}

impl RuleGroup {
    /// A group whose rules must all match.
    pub fn and(rules: impl IntoIterator<Item = impl Into<Rule>>) -> Self {
        Self::new(Combinator::And, rules)
    }

    /// A group of which one rule must match.
    pub fn or(rules: impl IntoIterator<Item = impl Into<Rule>>) -> Self {
        Self::new(Combinator::Or, rules)
    }

    /// A group with the given combinator.
    pub fn new(combinator: Combinator, rules: impl IntoIterator<Item = impl Into<Rule>>) -> Self {
        Self {
            combinator,
            rules: rules.into_iter().map(Into::into).collect(),
        }
    }

    /// The combinator.
    pub fn combinator(&self) -> Combinator {
        self.combinator
    }

    /// The rules, in order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

/// A condition or a nested group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// One test.
    Condition(Condition),
    /// A nested group.
    Group(RuleGroup),
}

impl From<Condition> for Rule {
    fn from(condition: Condition) -> Self {
        Self::Condition(condition)
    }
}

impl From<RuleGroup> for Rule {
    fn from(group: RuleGroup) -> Self {
        Self::Group(group)
    }
}
