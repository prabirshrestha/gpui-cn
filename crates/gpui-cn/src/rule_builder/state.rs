//! The rule tree a [`RuleBuilder`](super::RuleBuilder) edits, with the
//! select and text field entities of every condition row.

use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, EventEmitter, SharedString, Subscription,
    WeakEntity, Window,
};

use super::model::{Combinator, Condition, Rule, RuleEditor, RuleField, RuleGroup, RuleValue};
use crate::{InputEvent, InputState, SelectEvent, SelectItem, SelectState};

/// The id of a rule in a [`RuleBuilderState`]: given once, when the rule
/// is added, and kept while it moves, so it keys the rule's elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(u64);

impl RuleId {
    /// The number behind the id.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// What a [`RuleBuilderState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuleBuilderEvent {
    /// The query changed: a rule was added, removed, moved, or edited, or a
    /// combinator changed.
    Changed,
}

/// Where a custom value editor reports a row's value: a weak handle on
/// the state and the row's id. See [`RuleEditor::custom`].
#[derive(Clone)]
pub struct RuleValueSlot {
    state: WeakEntity<RuleBuilderState>,
    id: RuleId,
}

impl RuleValueSlot {
    /// The row the slot reports for.
    pub fn rule(&self) -> RuleId {
        self.id
    }

    /// Stores `value` as the row's value and emits
    /// [`RuleBuilderEvent::Changed`]. The editor's view stays as it is.
    ///
    /// The write happens once the current update ends, so a view may call
    /// it while it is built. It does nothing once the row or the state is
    /// gone, or once the row's field or operator no longer uses a custom
    /// editor that takes `value`.
    pub fn set(&self, value: RuleValue, cx: &mut App) {
        let (state, id) = (self.state.clone(), self.id);
        cx.defer(move |cx| {
            let _ = state.update(cx, |state, cx| state.store_custom_value(id, value, cx));
        });
    }
}

/// The field that takes a condition's value.
#[derive(Clone)]
pub(crate) enum ValueEditor {
    None,
    Text(Entity<InputState>),
    Choice(Entity<SelectState<SharedString>>),
    /// The field a new list value is typed in.
    List(Entity<InputState>),
    Custom(AnyView),
}

/// The controls of one condition row.
pub(crate) struct Editors {
    pub field: Entity<SelectState<SharedString>>,
    pub operator: Option<Entity<SelectState<SharedString>>>,
    pub value: ValueEditor,
    _field: Subscription,
    /// The operator's and the value's, replaced with them.
    _rest: Vec<Subscription>,
}

pub(crate) enum NodeKind {
    Condition {
        condition: Condition,
        editors: Box<Editors>,
    },
    Group {
        combinator: Combinator,
        children: Vec<Node>,
    },
}

pub(crate) struct Node {
    pub id: RuleId,
    pub kind: NodeKind,
}

impl Node {
    fn find(&self, id: RuleId) -> Option<&Node> {
        if self.id == id {
            return Some(self);
        }
        match &self.kind {
            NodeKind::Group { children, .. } => children.iter().find_map(|child| child.find(id)),
            NodeKind::Condition { .. } => None,
        }
    }

    fn find_mut(&mut self, id: RuleId) -> Option<&mut Node> {
        if self.id == id {
            return Some(self);
        }
        match &mut self.kind {
            NodeKind::Group { children, .. } => {
                children.iter_mut().find_map(|child| child.find_mut(id))
            }
            NodeKind::Condition { .. } => None,
        }
    }

    /// The group that holds `id`, and where in it.
    fn parent_of(&self, id: RuleId) -> Option<(RuleId, usize)> {
        let NodeKind::Group { children, .. } = &self.kind else {
            return None;
        };
        children.iter().enumerate().find_map(|(index, child)| {
            if child.id == id {
                Some((self.id, index))
            } else {
                child.parent_of(id)
            }
        })
    }

    fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match &mut self.kind {
            NodeKind::Group { children, .. } => Some(children),
            NodeKind::Condition { .. } => None,
        }
    }

    /// Moves the rules of each group that would sit `max` levels deep or
    /// more into the group above it. `depth` is this node's depth.
    fn limit_depth(&mut self, depth: usize, max: usize) {
        let NodeKind::Group { children, .. } = &mut self.kind else {
            return;
        };
        let mut kept = Vec::with_capacity(children.len());
        for mut child in std::mem::take(children) {
            let group = matches!(child.kind, NodeKind::Group { .. });
            if group && depth + 1 >= max {
                child.limit_depth(depth, max);
                if let NodeKind::Group {
                    children: inner, ..
                } = child.kind
                {
                    kept.extend(inner);
                }
            } else {
                child.limit_depth(depth + 1, max);
                kept.push(child);
            }
        }
        *children = kept;
    }

    /// How many group levels the subtree has below and including this node.
    fn height(&self) -> usize {
        match &self.kind {
            NodeKind::Group { children, .. } => {
                1 + children.iter().map(Node::height).max().unwrap_or(0)
            }
            NodeKind::Condition { .. } => 0,
        }
    }

    fn to_rule(&self) -> Rule {
        match &self.kind {
            NodeKind::Condition { condition, .. } => Rule::Condition(condition.clone()),
            NodeKind::Group {
                combinator,
                children,
            } => Rule::Group(RuleGroup::new(
                *combinator,
                children.iter().map(Node::to_rule),
            )),
        }
    }
}

/// The rules a [`RuleBuilder`](super::RuleBuilder) shows and edits: a root
/// group of conditions and nested groups over a set of [`RuleField`]s.
///
/// The view that shows the builder owns the state and observes it. Each
/// condition row keeps its own select and text field entities here, so
/// the builder renders without state of its own. Every mutation emits
/// [`RuleBuilderEvent::Changed`].
///
/// ```no_run
/// use gpui_cn::{Condition, RuleBuilderState, RuleEditor, RuleField, RuleGroup, RuleOperator};
/// use gpui_kit::{AppContext as _, Context, Window};
///
/// fn rules(window: &mut Window, cx: &mut Context<()>) {
///     let fields = [RuleField::new("branch", "Branch")
///         .operator(RuleOperator::new("eq", "Equal to"))
///         .editor(RuleEditor::text("main"))];
///     let _ = cx.new(|cx| {
///         RuleBuilderState::new(fields, window, cx)
///             .with_query(RuleGroup::and([Condition::new("branch")]), window, cx)
///     });
/// }
/// ```
pub struct RuleBuilderState {
    fields: Vec<RuleField>,
    root: Node,
    next_id: u64,
    max_depth: usize,
}

impl EventEmitter<RuleBuilderEvent> for RuleBuilderState {}

impl RuleBuilderState {
    /// An empty `And` group over `fields`, which nests one group deep.
    pub fn new(
        fields: impl IntoIterator<Item = RuleField>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self {
            fields: fields.into_iter().collect(),
            root: Node {
                id: RuleId(0),
                kind: NodeKind::Group {
                    combinator: Combinator::And,
                    children: Vec::new(),
                },
            },
            next_id: 1,
            max_depth: 2,
        }
    }

    /// Replaces the rules with `query`. A condition on a field the state
    /// does not have is dropped; a missing or unfit operator or value takes
    /// the field's default.
    pub fn with_query(
        mut self,
        query: RuleGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.root = self.build_group(RuleId(0), &query, 0, window, cx);
        self
    }

    /// How many group levels the tree may have, the root included: 1 keeps
    /// it flat, as in a list of actions. The default is 2.
    ///
    /// A group deeper than this, in the rules already held or in a later
    /// [`with_query`](Self::with_query), gives its rules to the group
    /// above it. Where the two combinators differ, that changes what the
    /// query means.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth.max(1);
        self.root.limit_depth(0, self.max_depth);
        self
    }

    /// The fields, in menu order.
    pub fn fields(&self) -> &[RuleField] {
        &self.fields
    }

    /// The field with `key`.
    pub fn field(&self, key: &str) -> Option<&RuleField> {
        self.fields.iter().find(|field| field.key().as_ref() == key)
    }

    /// The rules as plain data.
    pub fn query(&self) -> RuleGroup {
        match self.root.to_rule() {
            Rule::Group(group) => group,
            Rule::Condition(_) => unreachable!("the root is a group"),
        }
    }

    /// The root group.
    pub fn root(&self) -> RuleId {
        self.root.id
    }

    /// The ids of a group's rules, in order. Empty for a condition.
    pub fn children(&self, id: RuleId) -> Vec<RuleId> {
        match self.root.find(id).map(|node| &node.kind) {
            Some(NodeKind::Group { children, .. }) => {
                children.iter().map(|child| child.id).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The combinator of a group.
    pub fn combinator(&self, id: RuleId) -> Option<Combinator> {
        match self.root.find(id).map(|node| &node.kind) {
            Some(NodeKind::Group { combinator, .. }) => Some(*combinator),
            _ => None,
        }
    }

    /// The condition with `id`.
    pub fn condition(&self, id: RuleId) -> Option<&Condition> {
        match self.root.find(id).map(|node| &node.kind) {
            Some(NodeKind::Condition { condition, .. }) => Some(condition),
            _ => None,
        }
    }

    /// The group that holds `id`.
    pub fn parent(&self, id: RuleId) -> Option<RuleId> {
        self.root.parent_of(id).map(|(parent, _)| parent)
    }

    /// How many groups hold `id`: 0 for the root.
    pub fn depth(&self, id: RuleId) -> usize {
        let mut depth = 0;
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            depth += 1;
            current = parent;
        }
        depth
    }

    /// Whether a group can be added inside `group`.
    pub fn can_nest(&self, group: RuleId) -> bool {
        self.combinator(group).is_some() && self.depth(group) + 1 < self.max_depth
    }

    /// Adds a condition on the first field at the end of `group`.
    pub fn add_condition(
        &mut self,
        group: RuleId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<RuleId> {
        let field = self.fields.first()?.key().clone();
        let node = self.build_condition(Condition::new(field), window, cx)?;
        let id = node.id;
        self.root.find_mut(group)?.children_mut()?.push(node);
        self.changed(cx);
        Some(id)
    }

    /// Adds a group with one condition at the end of `group`, if the
    /// depth allows it.
    pub fn add_group(
        &mut self,
        group: RuleId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<RuleId> {
        if !self.can_nest(group) {
            return None;
        }
        let field = self.fields.first()?.key().clone();
        let query = RuleGroup::and([Condition::new(field)]);
        let id = self.next();
        let depth = self.depth(group) + 1;
        let node = self.build_group(id, &query, depth, window, cx);
        self.root.find_mut(group)?.children_mut()?.push(node);
        self.changed(cx);
        Some(id)
    }

    /// Removes a rule. A nested group left empty goes with it.
    pub fn remove(&mut self, id: RuleId, cx: &mut Context<Self>) {
        if self.take(id).is_some() {
            self.changed(cx);
        }
    }

    /// Sets the combinator of a group.
    pub fn set_combinator(&mut self, group: RuleId, value: Combinator, cx: &mut Context<Self>) {
        if let Some(Node {
            kind: NodeKind::Group { combinator, .. },
            ..
        }) = self.root.find_mut(group)
            && *combinator != value
        {
            *combinator = value;
            self.changed(cx);
        }
    }

    /// Switches a group between `And` and `Or`.
    pub fn toggle_combinator(&mut self, group: RuleId, cx: &mut Context<Self>) {
        if let Some(current) = self.combinator(group) {
            self.set_combinator(group, current.toggled(), cx);
        }
    }

    /// Moves a rule into `group` at `index`, counted after the rule leaves
    /// its place. A group never moves into itself, and a move that would
    /// nest deeper than the state allows is refused. Returns whether the
    /// rule moved.
    pub fn move_rule(
        &mut self,
        id: RuleId,
        group: RuleId,
        index: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(node) = self.root.find(id) else {
            return false;
        };
        if id == self.root.id || node.find(group).is_some() || self.combinator(group).is_none() {
            return false;
        }
        if self.depth(group) + node.height() >= self.max_depth {
            return false;
        }
        let Some((from, from_index)) = self.root.parent_of(id) else {
            return false;
        };
        let node = self
            .root
            .find_mut(from)
            .and_then(Node::children_mut)
            .map(|children| children.remove(from_index))
            .expect("the parent holds the rule");
        let target = self
            .root
            .find_mut(group)
            .and_then(Node::children_mut)
            .expect("the target is a group");
        let index = index.min(target.len());
        target.insert(index, node);
        self.prune(from);
        self.changed(cx);
        true
    }

    /// Moves a rule to just before `target`, in `target`'s group.
    pub fn move_before(&mut self, id: RuleId, target: RuleId, cx: &mut Context<Self>) -> bool {
        if id == target {
            return false;
        }
        let Some((group, mut index)) = self.root.parent_of(target) else {
            return false;
        };
        if let Some((from, from_index)) = self.root.parent_of(id)
            && from == group
            && from_index < index
        {
            index -= 1;
        }
        self.move_rule(id, group, index, cx)
    }

    /// Changes a condition's field. The operator and the value reset to
    /// the new field's defaults.
    pub fn set_field(
        &mut self,
        id: RuleId,
        key: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.field(&key).cloned() else {
            return;
        };
        let Some(Node {
            kind: NodeKind::Condition { condition, .. },
            ..
        }) = self.root.find_mut(id)
        else {
            return;
        };
        if condition.field() == &key {
            return;
        }
        *condition = Condition::new(key.clone());
        condition.normalize(&field);
        let condition = condition.clone();
        self.rebuild_rest(id, &field, &condition, window, cx);
        if let Some(editors) = self.editors(id) {
            editors
                .field
                .update(cx, |select, cx| select.set_selected([key], cx));
        }
        self.changed(cx);
    }

    /// Changes a condition's operator. The value stays when the new
    /// operator's editor takes it, and resets otherwise.
    pub fn set_operator(
        &mut self,
        id: RuleId,
        key: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self
            .condition(id)
            .and_then(|condition| self.field(condition.field()))
            .cloned()
        else {
            return;
        };
        if field.find_operator(&key).is_none() {
            return;
        }
        let Some(Node {
            kind: NodeKind::Condition { condition, .. },
            ..
        }) = self.root.find_mut(id)
        else {
            return;
        };
        if condition.operator() == Some(&key) {
            return;
        }
        condition.set_operator(Some(key));
        condition.normalize(&field);
        let condition = condition.clone();
        self.rebuild_rest(id, &field, &condition, window, cx);
        self.changed(cx);
    }

    /// Sets a condition's value, if its editor takes that kind of value.
    /// The row's value editor is built again with the new value, so a
    /// field that had focus loses it; a custom editor's view reports its
    /// own edits through its [`RuleValueSlot`] instead.
    pub fn set_value(
        &mut self,
        id: RuleId,
        value: RuleValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self
            .condition(id)
            .and_then(|condition| self.field(condition.field()))
            .cloned()
        else {
            return;
        };
        let Some(Node {
            kind: NodeKind::Condition { condition, .. },
            ..
        }) = self.root.find_mut(id)
        else {
            return;
        };
        let editor = field.editor_for(condition.operator().map(|op| op.as_ref()));
        if !editor.accepts(&value) || condition.value() == &value {
            return;
        }
        condition.set_value(value);
        condition.value_mut().dedupe();
        let condition = condition.clone();
        self.rebuild_rest(id, &field, &condition, window, cx);
        self.changed(cx);
    }

    /// Adds a value to the end of a list condition. Blank and repeated
    /// values are ignored.
    pub fn push_list_value(&mut self, id: RuleId, text: SharedString, cx: &mut Context<Self>) {
        let text: SharedString = text.trim().to_string().into();
        if text.is_empty() {
            return;
        }
        if let Some(RuleValue::List(values)) = self.value_mut(id)
            && !values.contains(&text)
        {
            values.push(text);
            self.changed(cx);
        }
    }

    /// Removes a value from a list condition.
    pub fn remove_list_value(&mut self, id: RuleId, text: &str, cx: &mut Context<Self>) {
        if let Some(RuleValue::List(values)) = self.value_mut(id) {
            let before = values.len();
            values.retain(|value| value.as_ref() != text);
            if values.len() != before {
                self.changed(cx);
            }
        }
    }

    /// Moves a rule `step` places within its group, such as -1 for one up.
    /// Returns whether it moved; a rule at the end it moves toward stays.
    pub fn move_by(&mut self, id: RuleId, step: isize, cx: &mut Context<Self>) -> bool {
        let Some((group, index)) = self.root.parent_of(id) else {
            return false;
        };
        let len = self.children(group).len();
        match index.checked_add_signed(step) {
            Some(target) if target < len => self.move_rule(id, group, target, cx),
            _ => false,
        }
    }

    /// The write behind [`RuleValueSlot::set`].
    fn store_custom_value(&mut self, id: RuleId, value: RuleValue, cx: &mut Context<Self>) {
        let Some(condition) = self.condition(id) else {
            return;
        };
        let Some(field) = self.field(condition.field()) else {
            return;
        };
        let editor = field.editor_for(condition.operator().map(|op| op.as_ref()));
        if !matches!(editor, RuleEditor::Custom { .. }) || !editor.accepts(&value) {
            return;
        }
        if let Some(current) = self.value_mut(id)
            && *current != value
        {
            *current = value;
            self.changed(cx);
        }
    }

    pub(crate) fn node(&self, id: RuleId) -> Option<&Node> {
        self.root.find(id)
    }

    pub(crate) fn editors(&self, id: RuleId) -> Option<&Editors> {
        match self.root.find(id).map(|node| &node.kind) {
            Some(NodeKind::Condition { editors, .. }) => Some(&**editors),
            _ => None,
        }
    }

    fn value_mut(&mut self, id: RuleId) -> Option<&mut RuleValue> {
        match self.root.find_mut(id).map(|node| &mut node.kind) {
            Some(NodeKind::Condition { condition, .. }) => Some(condition.value_mut()),
            _ => None,
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(RuleBuilderEvent::Changed);
        cx.notify();
    }

    fn next(&mut self) -> RuleId {
        let id = RuleId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Takes a rule out of the tree and drops the groups it leaves empty.
    fn take(&mut self, id: RuleId) -> Option<Node> {
        let (parent, index) = self.root.parent_of(id)?;
        let node = self.root.find_mut(parent)?.children_mut()?.remove(index);
        self.prune(parent);
        Some(node)
    }

    /// Removes `group` and the groups above it while each is empty, up to
    /// the root, which stays.
    fn prune(&mut self, group: RuleId) {
        let mut current = group;
        while current != self.root.id {
            let empty = matches!(
                self.root.find(current).map(|node| &node.kind),
                Some(NodeKind::Group { children, .. }) if children.is_empty()
            );
            if !empty {
                return;
            }
            let Some((parent, index)) = self.root.parent_of(current) else {
                return;
            };
            if let Some(children) = self.root.find_mut(parent).and_then(Node::children_mut) {
                children.remove(index);
            }
            current = parent;
        }
    }

    /// Builds a group `depth` groups below the root. A nested group that
    /// would pass the state's depth gives its rules to this group instead.
    fn build_group(
        &mut self,
        id: RuleId,
        query: &RuleGroup,
        depth: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Node {
        let mut children = Vec::new();
        self.build_rules(query, depth, &mut children, window, cx);
        Node {
            id,
            kind: NodeKind::Group {
                combinator: query.combinator(),
                children,
            },
        }
    }

    fn build_rules(
        &mut self,
        query: &RuleGroup,
        depth: usize,
        children: &mut Vec<Node>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for rule in query.rules() {
            match rule {
                Rule::Condition(condition) => {
                    if let Some(node) = self.build_condition(condition.clone(), window, cx) {
                        children.push(node);
                    }
                }
                Rule::Group(group) if depth + 1 >= self.max_depth => {
                    self.build_rules(group, depth, children, window, cx);
                }
                Rule::Group(group) => {
                    let child = self.next();
                    let node = self.build_group(child, group, depth + 1, window, cx);
                    if matches!(&node.kind, NodeKind::Group { children, .. } if !children.is_empty())
                    {
                        children.push(node);
                    }
                }
            }
        }
    }

    fn build_condition(
        &mut self,
        mut condition: Condition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Node> {
        let field = self.field(condition.field())?.clone();
        condition.normalize(&field);
        let id = self.next();
        let items = self.fields.iter().map(|field| {
            let item = SelectItem::new(field.key().clone(), field.label().clone());
            match field.icon_source() {
                Some(icon) => item.icon(icon.clone()),
                None => item,
            }
        });
        let items: Vec<_> = items.collect();
        let key = condition.field().clone();
        let select = cx.new(|cx| SelectState::new(items, cx).with_selected([key]));
        let field_subscription = cx.subscribe_in(
            &select,
            window,
            move |this, select, event: &SelectEvent<SharedString>, window, cx| {
                // The menu's selection now, not the event's: `set_field`
                // also selects in the menu, and acting on queued events
                // in turn would swing the field back and forth.
                if let SelectEvent::Changed(_) = event
                    && let Some(key) = select.read(cx).selected_value().cloned()
                {
                    this.set_field(id, key, window, cx);
                }
            },
        );
        let (operator, value, rest) = Self::build_rest(id, &field, &condition, window, cx);
        Some(Node {
            id,
            kind: NodeKind::Condition {
                condition,
                editors: Box::new(Editors {
                    field: select,
                    operator,
                    value,
                    _field: field_subscription,
                    _rest: rest,
                }),
            },
        })
    }

    /// Replaces a row's operator and value editors after its field or
    /// operator changed.
    fn rebuild_rest(
        &mut self,
        id: RuleId,
        field: &RuleField,
        condition: &Condition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (operator, value, rest) = Self::build_rest(id, field, condition, window, cx);
        if let Some(Node {
            kind: NodeKind::Condition { editors, .. },
            ..
        }) = self.root.find_mut(id)
        {
            editors.operator = operator;
            editors.value = value;
            editors._rest = rest;
        }
    }

    fn build_rest(
        id: RuleId,
        field: &RuleField,
        condition: &Condition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (
        Option<Entity<SelectState<SharedString>>>,
        ValueEditor,
        Vec<Subscription>,
    ) {
        let mut subscriptions = Vec::new();
        let operator = (field.operators().len() > 1).then(|| {
            let items: Vec<_> = field
                .operators()
                .iter()
                .map(|op| SelectItem::new(op.key().clone(), op.label().clone()))
                .collect();
            let selected = condition.operator().cloned();
            let select = cx.new(|cx| SelectState::new(items, cx).with_selected(selected));
            subscriptions.push(cx.subscribe_in(
                &select,
                window,
                move |this, _, event: &SelectEvent<SharedString>, window, cx| {
                    if let SelectEvent::Changed(values) = event
                        && let Some(key) = values.first()
                    {
                        this.set_operator(id, key.clone(), window, cx);
                    }
                },
            ));
            select
        });
        let editor = field.editor_for(condition.operator().map(|op| op.as_ref()));
        let value = match editor {
            RuleEditor::None => ValueEditor::None,
            RuleEditor::Text { placeholder } => {
                let text = match condition.value() {
                    RuleValue::Text(text) => text.clone(),
                    _ => SharedString::default(),
                };
                let placeholder = placeholder.clone();
                let input = cx.new(|cx| {
                    let mut input = InputState::new(window, cx).placeholder(placeholder);
                    input.set_value(text, window, cx);
                    input
                });
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            let text = input.read(cx).value();
                            if let Some(value @ RuleValue::Text(_)) = this.value_mut(id)
                                && *value != RuleValue::Text(text.clone())
                            {
                                *value = RuleValue::Text(text);
                                this.changed(cx);
                            }
                        }
                    },
                ));
                ValueEditor::Text(input)
            }
            RuleEditor::Choice(items) => {
                let selected = match condition.value() {
                    RuleValue::Choice(key) => Some(key.clone()),
                    _ => None,
                };
                let items = items.clone();
                let select = cx.new(|cx| SelectState::new(items, cx).with_selected(selected));
                subscriptions.push(cx.subscribe_in(
                    &select,
                    window,
                    move |this, _, event: &SelectEvent<SharedString>, _, cx| {
                        if let SelectEvent::Changed(values) = event
                            && let Some(key) = values.first()
                            && let Some(value @ RuleValue::Choice(_)) = this.value_mut(id)
                            && *value != RuleValue::Choice(key.clone())
                        {
                            *value = RuleValue::Choice(key.clone());
                            this.changed(cx);
                        }
                    },
                ));
                ValueEditor::Choice(select)
            }
            RuleEditor::List { placeholder, .. } => {
                let placeholder = placeholder.clone();
                let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event: &InputEvent, window, cx| {
                        if matches!(event, InputEvent::PressEnter { .. }) {
                            let text = input.read(cx).value();
                            this.push_list_value(id, text, cx);
                            input.update(cx, |input, cx| {
                                input.set_value(SharedString::default(), window, cx)
                            });
                        }
                    },
                ));
                ValueEditor::List(input)
            }
            RuleEditor::Custom { build, .. } => {
                let slot = RuleValueSlot {
                    state: cx.entity().downgrade(),
                    id,
                };
                ValueEditor::Custom(build(condition.value(), slot, window, cx))
            }
        };
        (operator, value, subscriptions)
    }
}
