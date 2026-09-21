use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui_cn::{
    ActiveTheme as _, Select, SelectEntry, SelectItem, SelectRow, SelectState, SelectValue,
    Skeleton, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, Hsla, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Task, Window, base::Disableable as _,
    div, hsla, prelude::FluentBuilder as _, px,
};

use crate::{PAGE_WIDTH, Story, note, page, section};

/// The select in every shape the reference app uses it.
pub struct SelectStory {
    shortcut: Entity<SelectState<&'static str>>,
    app: Entity<SelectState<&'static str>>,
    accent: Entity<SelectState<&'static str>>,
    policy: Entity<SelectState<&'static str>>,
    filter: Entity<SelectState<&'static str>>,
    language: Entity<SelectState<&'static str>>,
    efforts: Entity<SelectState<Effort>>,
    numbers: Entity<SelectState<usize>>,
    cities: Entity<SelectState<usize>>,
    assignee: Entity<SelectState<usize>>,
    team: Entity<SelectState<usize>>,
    disabled: Entity<SelectState<&'static str>>,
}

/// People a custom row renderer draws with an initials avatar, the name,
/// and the address.
const PEOPLE: [(&str, &str); 4] = [
    ("Ada Lovelace", "ada@example.com"),
    ("Grace Hopper", "grace@example.com"),
    ("Linus Torvalds", "linus@example.com"),
    ("Margaret Hamilton", "margaret@example.com"),
];

/// The first letter of each of the first two words.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .collect()
}

/// A value of the application's own: a select is generic over its value,
/// and an enum needs only a stable key per variant.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Effort {
    Light,
    Medium,
    High,
    ExtraHigh,
    Max,
}

impl SelectValue for Effort {
    fn key(&self) -> SharedString {
        match self {
            Self::Light => "light".into(),
            Self::Medium => "medium".into(),
            Self::High => "high".into(),
            Self::ExtraHigh => "xhigh".into(),
            Self::Max => "max".into(),
        }
    }
}

/// Cities a search handler looks through on a background thread.
const CITIES: [&str; 24] = [
    "Amsterdam",
    "Athens",
    "Bangkok",
    "Berlin",
    "Bogota",
    "Boston",
    "Buenos Aires",
    "Cairo",
    "Chicago",
    "Copenhagen",
    "Dublin",
    "Hanoi",
    "Helsinki",
    "Kathmandu",
    "Lisbon",
    "London",
    "Madrid",
    "Nairobi",
    "Oslo",
    "Seattle",
    "Seoul",
    "Tokyo",
    "Vienna",
    "Zurich",
];

/// Whether every character of `query` appears in `text` in order,
/// ignoring case: the loosest kind of fuzzy match.
fn fuzzy(text: &str, query: &str) -> bool {
    let mut chars = text.chars().flat_map(char::to_lowercase);
    query
        .chars()
        .flat_map(char::to_lowercase)
        .all(|wanted| chars.by_ref().any(|c| c == wanted))
}

impl SelectStory {
    /// The menus a headless run opens in turn, each by its select's id
    /// with the query it types once the menu is open: every menu shape
    /// the page has, and the searchable ones narrowed.
    pub const MENUS: &'static [(&'static str, &'static str)] = &[
        ("shortcut", ""),
        ("filter", ""),
        ("policy", ""),
        ("efforts", ""),
        ("language", ""),
        ("language", "an"),
        ("numbers", ""),
        ("cities", "an"),
        ("assignee", ""),
        ("team", "ada"),
    ];
}

impl Story for SelectStory {
    fn title() -> &'static str {
        "Select"
    }

    fn icon() -> IconName {
        IconName::ChevronsUpDown
    }

    fn description() -> &'static str {
        "Displays a list of options for the user to pick from, triggered by a button."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| Self::new(window, cx)).into()
    }
}

/// The accent colors the reference app offers, with a swatch before each.
const ACCENTS: [(&str, &str, f32); 7] = [
    ("default", "Default", -1.),
    ("blue", "Blue", 0.6),
    ("green", "Green", 0.36),
    ("yellow", "Yellow", 0.13),
    ("pink", "Pink", 0.9),
    ("orange", "Orange", 0.07),
    ("purple", "Purple", 0.75),
];

/// Languages with their English names as keywords, so a search in
/// either finds them.
const LANGUAGES: [(&str, &str, &str); 16] = [
    ("auto", "Auto detect", ""),
    ("sq", "Albanian", ""),
    ("hy", "Armenian", ""),
    ("ms", "Bahasa Melayu", "malay"),
    ("bs", "bosanski", "bosnian"),
    ("my", "Burmese", ""),
    ("ca", "catala", "catalan"),
    ("cs", "cestina", "czech"),
    ("da", "dansk", "danish"),
    ("de", "Deutsch", "german"),
    ("et", "eesti", "estonian"),
    ("en", "English", ""),
    ("es", "espanol", "spanish"),
    ("fr", "francais", "french"),
    ("it", "italiano", "italian"),
    ("ja", "Japanese", "nihongo"),
];

impl SelectStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shortcut = cx.new(|cx| {
            SelectState::new(
                [
                    SelectItem::new("enter", "Enter"),
                    SelectItem::new("cmd-enter-multiline", "Cmd + Enter for multiline prompts"),
                    SelectItem::new("cmd-enter", "Cmd + Enter always"),
                ],
                cx,
            )
            .with_selected(["enter"])
        });
        let app = cx.new(|cx| {
            SelectState::new(
                [
                    SelectItem::new("code", "VS Code").icon(IconName::SquareTerminal),
                    SelectItem::new("default", "Default app").icon(IconName::File),
                    SelectItem::new("finder", "Finder").icon(IconName::Folder),
                    SelectItem::new("terminal", "Terminal").icon(IconName::SquareTerminal),
                    SelectItem::new("browser", "Browser").icon(IconName::Globe),
                ],
                cx,
            )
            .with_selected(["finder"])
        });
        let accent = cx.new(|cx| {
            let swatches = ACCENTS.iter().map(|(value, label, hue)| {
                let hue = *hue;
                SelectItem::new(*value, *label).leading(move |_, cx| swatch(hue, cx))
            });
            SelectState::new(
                swatches
                    .map(SelectEntry::from)
                    .chain([SelectItem::new("custom", "Custom").into()]),
                cx,
            )
            .with_selected(["default"])
        });
        let policy = cx.new(|cx| {
            SelectState::new(
                [
                    SelectItem::new("on-request", "On request")
                        .description("Ask when escalation is requested"),
                    SelectItem::new("never", "Never ask for approval")
                        .description("Blocked actions fail instead of requesting approval"),
                ],
                cx,
            )
            .with_selected(["on-request"])
        });
        let filter = cx.new(|cx| {
            SelectState::new(
                [
                    SelectEntry::label("Type"),
                    SelectItem::new("all", "All chats").into(),
                    SelectItem::new("local", "Local").into(),
                    SelectItem::new("cloud", "Cloud").into(),
                    SelectEntry::Separator,
                    SelectEntry::label("Sort by"),
                    SelectItem::new("updated", "Updated").into(),
                    SelectItem::new("created", "Created").into(),
                    SelectItem::new("alphabetical", "Alphabetical").into(),
                ],
                cx,
            )
            .with_selected(["all"])
        });
        let language = cx.new(|cx| {
            SelectState::new(
                LANGUAGES.iter().map(|(value, label, keyword)| {
                    let item = SelectItem::new(*value, *label);
                    if keyword.is_empty() {
                        item
                    } else {
                        item.keywords([*keyword])
                    }
                }),
                cx,
            )
            .with_selected(["auto"])
            .with_search("Search languages", window, cx)
        });
        let efforts = cx.new(|cx| {
            SelectState::multiple(
                [
                    SelectItem::new(Effort::Light, "Light"),
                    SelectItem::new(Effort::Medium, "Medium"),
                    SelectItem::new(Effort::High, "High"),
                    SelectItem::new(Effort::ExtraHigh, "Extra High"),
                    SelectItem::new(Effort::Max, "Max").disabled(true),
                ],
                cx,
            )
            .with_selected([
                Effort::Light,
                Effort::Medium,
                Effort::High,
                Effort::ExtraHigh,
            ])
        });
        let numbers = cx.new(|cx| {
            SelectState::new(
                (1..=10_000).map(|n| SelectItem::new(n, format!("Item {n}"))),
                cx,
            )
            .with_search("Search 10,000 items", window, cx)
        });
        let cities = cx.new(|cx| {
            let answered: Rc<RefCell<HashMap<String, Vec<SelectEntry<usize>>>>> = Rc::default();
            SelectState::new(Vec::<SelectEntry<usize>>::new(), cx).with_search_handler(
                "Search cities",
                move |query, cx| {
                    if let Some(rows) = answered.borrow().get(query.as_ref()) {
                        return Task::ready(rows.clone());
                    }
                    let timer = cx.background_executor().timer(Duration::from_millis(300));
                    let answered = answered.clone();
                    let key = query.to_string();
                    let rows = cx.background_spawn(async move {
                        timer.await;
                        CITIES
                            .iter()
                            .enumerate()
                            .filter(|(_, city)| fuzzy(city, &query))
                            .map(|(index, city)| SelectItem::new(index, *city).into())
                            .collect::<Vec<SelectEntry<usize>>>()
                    });
                    cx.spawn(async move |_, _| {
                        let rows = rows.await;
                        answered.borrow_mut().insert(key, rows.clone());
                        rows
                    })
                },
                window,
                cx,
            )
        });
        let assignee = cx.new(|cx| {
            SelectState::new(
                PEOPLE.iter().enumerate().map(|(index, (name, email))| {
                    SelectItem::new(index, *name).description(*email)
                }),
                cx,
            )
            .with_selected([1])
        });
        let team = cx.new(|cx| {
            SelectState::new(
                (0..2_000).map(|n| {
                    let (name, _) = PEOPLE[n % PEOPLE.len()];
                    SelectItem::new(n, format!("{name} {}", n + 1))
                        .description(format!("member{}@example.com", n + 1))
                }),
                cx,
            )
            .with_search("Search 2,000 members", window, cx)
            .with_row_height(px(48.))
        });
        let disabled = cx.new(|cx| {
            SelectState::new([SelectItem::new("system", "System default")], cx)
                .with_selected(["system"])
        });
        for state in [
            &shortcut, &app, &accent, &policy, &filter, &language, &disabled,
        ] {
            cx.observe(state, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&efforts, |_, _, cx| cx.notify()).detach();
        cx.observe(&numbers, |_, _, cx| cx.notify()).detach();
        cx.observe(&cities, |_, _, cx| cx.notify()).detach();
        cx.observe(&assignee, |_, _, cx| cx.notify()).detach();
        cx.observe(&team, |_, _, cx| cx.notify()).detach();
        Self {
            shortcut,
            app,
            accent,
            policy,
            filter,
            language,
            efforts,
            numbers,
            cities,
            assignee,
            team,
            disabled,
        }
    }
}

/// A 12px dot in the accent, or a ring for the default.
fn swatch(hue: f32, cx: &App) -> gpui_kit::Div {
    let theme = cx.theme();
    let color: Hsla = if hue < 0. {
        theme.muted_foreground()
    } else {
        hsla(hue, 0.6, 0.55, 1.)
    };
    div()
        .size_4()
        .flex()
        .items_center()
        .justify_center()
        .child(div().size_3().rounded_full().bg(color))
}

/// A row of the assignee menu: an avatar of initials in the accent, the
/// name over the address, and a check when chosen.
fn person_row(item: &SelectItem<usize>, row: SelectRow, cx: &App) -> gpui_kit::Div {
    let theme = cx.theme();
    let avatar = div()
        .flex_shrink_0()
        .size_6()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.ring())
        .text_color(theme.primary_foreground())
        .text_xs()
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .child(initials(item.label()));
    div()
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .child(avatar)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(item.label().clone())
                .when_some(item.description_text().cloned(), |this, email| {
                    this.child(
                        div()
                            .text_color(theme.popover_muted_foreground)
                            .text_xs()
                            .child(email),
                    )
                }),
        )
        .when(row.is_selected(), |this| {
            this.child(
                gpui_cn::Icon::from(IconName::Check)
                    .size_4()
                    .text_color(theme.select_indicator),
            )
        })
}

/// The assignee trigger: the avatar and the name, or "Unassigned".
fn person_value(items: &[SelectItem<usize>], cx: &App) -> gpui_kit::Div {
    let theme = cx.theme();
    let row = div().flex().items_center().gap_1p5();
    match items.first() {
        Some(item) => row
            .child(
                div()
                    .flex_shrink_0()
                    .size_4()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.ring())
                    .text_color(theme.primary_foreground())
                    .text_size(theme.base.typography.xs.size)
                    .line_height(theme.base.typography.xs.size)
                    .child(initials(item.label()).chars().take(1).collect::<String>()),
            )
            .child(item.label().clone()),
        None => row.text_color(theme.muted_foreground()).child("Unassigned"),
    }
}

/// What the city menu shows while its handler is out: skeleton rows the
/// height of the ones to come, so the menu keeps its size and the answer
/// fills it in rather than growing it.
fn searching_rows(cx: &App) -> gpui_kit::Div {
    let theme = cx.theme();
    let fill = theme.popover_accent;
    let row_height = theme.metrics.row_sm;
    let padding = theme.metrics.control_padding_md;
    let text = theme.text_control.size;
    div().flex().flex_col().children((0..6usize).map(|n| {
        div().h(row_height).px(padding).flex().items_center().child(
            Skeleton::new(("city", n))
                .h(text)
                .w(gpui_kit::relative(0.45 + (n % 3) as f32 * 0.15))
                .bg(fill),
        )
    }))
}

/// What the city menu shows when nothing matches: the query itself.
fn no_city_row(cities: &Entity<SelectState<usize>>, cx: &App) -> gpui_kit::Div {
    let theme = cx.theme();
    let query = cities.read(cx).query(cx);
    div()
        .py_6()
        .text_center()
        .text_color(theme.muted_foreground())
        .child(format!("No city named \"{query}\""))
}

/// "Selected: " and the keys of the selection, or "nothing".
fn summary<V: SelectValue>(state: &Entity<SelectState<V>>, cx: &App) -> SharedString {
    let keys: Vec<String> = state
        .read(cx)
        .selected_items()
        .iter()
        .map(|item| item.key().to_string())
        .collect();
    if keys.is_empty() {
        "Selected: nothing".into()
    } else {
        format!("Selected: {}", keys.join(", ")).into()
    }
}

impl Render for SelectStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let control = cx.theme().text_control;
        let row = |label: &'static str, hint: SharedString, select: AnyElement| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .w_full()
                .max_w(PAGE_WIDTH)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(control.size)
                                .line_height(control.line_height)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(control.size)
                                .line_height(control.line_height)
                                .text_color(cx.theme().muted_foreground())
                                .child(hint),
                        ),
                )
                .child(select)
        };
        page([
            section(
                "Single",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "Tab to a trigger and press Down, Enter, or Space to open it. The \
                         arrows walk the rows, Enter chooses, Escape closes.",
                        cx,
                    ))
                    .child(row(
                        "Send shortcut",
                        summary(&self.shortcut, cx),
                        Select::new("shortcut", &self.shortcut)
                            .accessibility_label("Send shortcut")
                            .into_any_element(),
                    ))
                    .child(row(
                        "Open files with",
                        "Rows and the trigger show the item's icon.".into(),
                        Select::new("app", &self.app)
                            .accessibility_label("Open files with")
                            .into_any_element(),
                    ))
                    .child(row(
                        "Accent",
                        "Any element can lead a row; the last row has none.".into(),
                        Select::new("accent", &self.accent)
                            .accessibility_label("Accent")
                            .into_any_element(),
                    ))
                    .child(row(
                        "Approval policy",
                        "A description under the label.".into(),
                        Select::new("policy", &self.policy)
                            .accessibility_label("Approval policy")
                            .into_any_element(),
                    ))
                    .child(row(
                        "Archived chats",
                        "Group labels and separators.".into(),
                        Select::new("filter", &self.filter)
                            .icon(IconName::Inbox)
                            .accessibility_label("Filter")
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Searchable",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "A search field filters the rows by label and keywords: \"german\" \
                         finds Deutsch. The list scrolls past the menu's height.",
                        cx,
                    ))
                    .child(row(
                        "Language",
                        summary(&self.language, cx),
                        Select::new("language", &self.language)
                            .accessibility_label("Language")
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Multiple",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "Rows toggle and the menu stays open. The trigger counts the \
                         selection. Max is disabled.",
                        cx,
                    ))
                    .child(row(
                        "Available reasoning efforts",
                        summary(&self.efforts, cx),
                        Select::new("efforts", &self.efforts)
                            .accessibility_label("Available reasoning efforts")
                            .placeholder("None")
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Virtualized",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "Ten thousand rows in a virtual list that lays out only the rows in \
                         view, with a fixed menu width.",
                        cx,
                    ))
                    .child(row(
                        "Item",
                        summary(&self.numbers, cx),
                        Select::new("numbers", &self.numbers)
                            .menu_width(px(240.))
                            .placeholder("Pick an item")
                            .accessibility_label("Item")
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Async search",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "A search handler answers each query itself: here a fuzzy match over \
                         cities on a background thread, after a pause that stands in for a \
                         request. A newer query cancels the one before it, the chosen city \
                         stays chosen when the rows change, and the menu draws skeleton rows \
                         while it waits and a no-match row that names the query.",
                        cx,
                    ))
                    .child(row(
                        "City",
                        summary(&self.cities, cx),
                        Select::new("cities", &self.cities)
                            .menu_width(px(240.))
                            .placeholder("Pick a city")
                            .accessibility_label("City")
                            .render_loading(|_, cx| searching_rows(cx))
                            .render_empty({
                                let cities = self.cities.clone();
                                move |_, cx| no_city_row(&cities, cx)
                            })
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Custom rows",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(note(
                        "A renderer draws the inside of every row: an initials avatar, the \
                         name over the address, and its own check. The row's highlight and \
                         click stay the menu's. Assignee's rows are as tall as they draw; \
                         Team fixes every row at one height, so a scroll through its two \
                         thousand virtual rows lands exactly.",
                        cx,
                    ))
                    .child(row(
                        "Assignee",
                        summary(&self.assignee, cx),
                        Select::new("assignee", &self.assignee)
                            .menu_width(px(280.))
                            .placeholder("Unassigned")
                            .accessibility_label("Assignee")
                            .render_item(|item, row, _, cx| person_row(item, row, cx))
                            .render_value(|items, _, cx| person_value(items, cx))
                            .into_any_element(),
                    ))
                    .child(row(
                        "Team",
                        summary(&self.team, cx),
                        Select::new("team", &self.team)
                            .menu_width(px(280.))
                            .placeholder("Pick a member")
                            .accessibility_label("Team")
                            .render_item(|item, row, _, cx| person_row(item, row, cx))
                            .into_any_element(),
                    )),
            )
            .into_any_element(),
            section(
                "Disabled",
                div().flex().flex_col().gap_4().w_full().child(row(
                    "Microphone",
                    "Keeps its value, takes no input.".into(),
                    Select::new("disabled", &self.disabled)
                        .disabled(true)
                        .accessibility_label("Microphone")
                        .into_any_element(),
                )),
            )
            .into_any_element(),
        ])
    }
}
