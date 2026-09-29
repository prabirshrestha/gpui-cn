//! A dialog to choose a folder, on any file system: the path is typed
//! text, and the folders of its directory are listed, filtered fuzzily by
//! what follows the last separator.
//!
//! [`FolderPickerState`] holds the path, the listings, and the filtered
//! rows; [`FolderPicker`] draws them in a [`Dialog`]. Nothing waits on
//! the UI thread: a [`FolderSource`] returns a task for each page of a
//! listing, and the filter runs on a background task that the next
//! keystroke cancels.

mod path;
mod source;

use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    KeyBinding, ListAlignment, ListState, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, Task, Window, actions,
    assets::IconName,
    base::{
        Disableable as _, TestSupportExt as _,
        actions::{Confirm, SelectDown, SelectUp},
        h_flex,
        input::{InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};

use path::{Match, directory_path, filter, join_dir, parent_text, resolve_descend, split_path};
pub use source::{FolderEntry, FolderPage, FolderSource, LocalFolders, PageToken};

use crate::{ActiveTheme as _, Button, Dialog, Icon, ScrollArea, Spinner, Theme, menu::MenuLook};

/// The key context of the picker, which takes Up, Down, and Enter the
/// path field passes on.
const CONTEXT: &str = "GpuiCnFolderPicker";

const DEFAULT_TITLE: &str = "Choose a folder";
const EMPTY: &str = "No folders found in this directory.";
const FAILED: &str = "Unable to load this folder";
const NO_MATCH: &str = "No folders match.";
const LOAD_MORE: &str = "Load more";
const LOADING_MORE: &str = "Loading more";
const MORE_FAILED: &str = "Unable to load more. Try again.";

actions!(
    gpui_cn_folder_picker,
    [
        /// Chooses the folder, as the "Use folder" button does.
        Submit
    ]
);

/// The shortcut that submits: Cmd+Enter on macOS, Ctrl+Enter elsewhere.
const SUBMIT_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-enter"
} else {
    "ctrl-enter"
};

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(SUBMIT_KEY, Submit, Some(CONTEXT)),
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
    ]);
}

/// Where the request for the next page of a listing stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MoreState {
    /// No request is running. Reaching the end of the list starts one.
    Idle,
    /// The next page is on its way.
    Loading,
    /// The last request failed. The folders loaded stay, and the "Load
    /// more" row asks again.
    Failed,
}

/// The folders of a directory loaded so far, and whether more follow.
#[derive(Debug)]
#[non_exhaustive]
pub struct Loaded {
    entries: Arc<Vec<FolderEntry>>,
    names: HashSet<SharedString>,
    next: Option<PageToken>,
    more: MoreState,
}

impl Loaded {
    fn from_page(page: FolderPage) -> Self {
        let mut loaded = Self {
            entries: Arc::new(Vec::new()),
            names: HashSet::new(),
            next: None,
            more: MoreState::Idle,
        };
        loaded.append(page);
        loaded
    }

    /// Adds a page: its folders, less the names already loaded, and where
    /// the following page starts.
    fn append(&mut self, page: FolderPage) {
        self.next = page.next().cloned();
        let entries = Arc::make_mut(&mut self.entries);
        for entry in page.entries().iter().cloned() {
            if self.names.insert(entry.name().clone()) {
                entries.push(entry);
            }
        }
        self.more = MoreState::Idle;
    }

    /// The folders loaded, in the order the source gave them.
    pub fn entries(&self) -> &[FolderEntry] {
        &self.entries
    }

    /// Where the next page starts, when the source has more.
    pub fn next(&self) -> Option<&PageToken> {
        self.next.as_ref()
    }

    /// How the request for the next page stands.
    pub fn more(&self) -> MoreState {
        self.more
    }
}

/// What a [`FolderPickerState`] knows about one directory.
#[derive(Debug)]
#[non_exhaustive]
pub enum Listing {
    /// The source has not answered for the first page yet.
    Loading,
    /// The folders loaded so far.
    Ready(Loaded),
    /// The source failed for the first page, such as for a path that does
    /// not exist.
    Failed,
}

/// What a [`FolderPickerState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FolderPickerEvent {
    /// A folder was chosen. The path has no trailing separator, except
    /// for a root.
    Chosen(PathBuf),
    /// The picker was cancelled.
    Cancelled,
}

/// The rows the list shows: every folder in order, for an empty query,
/// or the folders that match it, ranked.
enum Shown {
    All(usize),
    Ranked(Vec<Match>),
}

impl Shown {
    fn len(&self) -> usize {
        match self {
            Self::All(len) => *len,
            Self::Ranked(matches) => matches.len(),
        }
    }
}

/// What the picker's list area shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Body {
    Loading,
    Failed,
    Empty,
    NoMatch,
    List,
}

/// A separator typed after a query, waiting for the filter of that query
/// to finish so it knows which folder to go into.
struct PendingDescend {
    text: String,
}

/// The path text, the listings, and the filtered rows of a
/// [`FolderPicker`].
///
/// The text is an absolute path. Everything up to and including its last
/// separator is the directory that is listed; what follows is a fuzzy
/// query over that directory's folders. Typing a separator after a
/// query goes inside the folder named exactly like it, or else the best
/// match. Enter or a click on a row goes inside it too, and the Up
/// button goes to the parent.
///
/// A source can list a directory in pages. While more follow, the list
/// ends with a "Load more" row, and reaching it loads the next page. The
/// query filters what is loaded, so a match on a later page shows once
/// that page is loaded.
///
/// Every listing is cached, so going back is instant. An answer for a
/// directory the path has left is dropped, and a new query cancels the
/// filter before it.
pub struct FolderPickerState {
    input: Entity<InputState>,
    source: Rc<dyn FolderSource>,
    windows: bool,
    window: AnyWindowHandle,
    dir_text: String,
    dir: PathBuf,
    query: SharedString,
    listings: HashMap<PathBuf, Listing>,
    /// The running listing of the first page. Dropping it cancels it.
    load: Option<Task<()>>,
    /// The running request for a later page. Dropping it cancels it.
    load_more: Option<Task<()>>,
    /// Which directory visit the running requests belong to.
    load_id: u64,
    shown: Shown,
    highlighted: Option<usize>,
    list: ListState,
    /// How many items the list was told it has.
    list_count: usize,
    /// The running filter. Dropping it cancels it.
    filter: Option<Task<()>>,
    filter_id: u64,
    pending: Option<PendingDescend>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<FolderPickerEvent> for FolderPickerState {}

/// The path field's focus, which holds the keyboard for the picker.
impl Focusable for FolderPickerState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl FolderPickerState {
    /// A picker on the local file system, at the root. Seed it with
    /// [`with_initial`](Self::with_initial).
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Path"));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    this.text_changed(text, window, cx);
                }
            },
        );
        let overdraw = cx.theme().metrics.folder_list_height;
        let list = ListState::new(0, ListAlignment::Top, overdraw);
        let weak = cx.weak_entity();
        list.set_scroll_handler(move |event, _, cx| {
            if event.count > 0 && event.visible_range.end >= event.count {
                weak.update(cx, |state, cx| state.load_more_on_scroll(cx))
                    .ok();
            }
        });
        let mut state = Self {
            input,
            source: Rc::new(LocalFolders),
            windows: cfg!(windows),
            window: window.window_handle(),
            dir_text: String::new(),
            dir: PathBuf::new(),
            query: SharedString::default(),
            listings: HashMap::new(),
            load: None,
            load_more: None,
            load_id: 0,
            shown: Shown::All(0),
            highlighted: None,
            list,
            list_count: 0,
            filter: None,
            filter_id: 0,
            pending: None,
            _subscriptions: vec![subscription],
        };
        state.set_text("/", window, cx);
        state
    }

    /// Lists folders from `source` instead of the local file system. The
    /// listings cached so far are dropped and the current directory is
    /// listed again. See [`FolderSource`] for how to write one.
    pub fn with_source(mut self, source: impl FolderSource, cx: &mut Context<Self>) -> Self {
        self.source = Rc::new(source);
        self.listings.clear();
        self.load = None;
        self.load_more = None;
        self.dir = PathBuf::new();
        let text = self.input.read(cx).value().to_string();
        self.apply_text(text, cx);
        self
    }

    /// Starts in `path`, which is listed at once.
    pub fn with_initial(
        mut self,
        path: impl AsRef<Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut text = path.as_ref().to_string_lossy().into_owned();
        if !text.ends_with(|c| path::is_separator(c, self.windows)) {
            text.push(path::separator(self.windows));
        }
        self.set_text(&text, window, cx);
        self
    }

    /// The path field's state.
    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    /// The directory that is listed.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The query: the text after the last separator.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The path text, as the field shows it.
    pub fn text(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    /// What is known about the directory that is listed.
    pub fn listing(&self) -> Option<&Listing> {
        self.listings.get(&self.dir)
    }

    /// How many folders the list shows, not counting the "Load more"
    /// row.
    pub fn row_count(&self) -> usize {
        self.shown.len()
    }

    /// The name of the folder the keyboard is on.
    pub fn highlighted(&self) -> Option<SharedString> {
        self.entry_at(self.highlighted?)
            .map(|(entry, _)| entry.name().clone())
    }

    /// The folder "Use folder" would choose: the listed directory while
    /// the query is empty, or the folder the query names exactly. `None`
    /// while the directory is loading or failed, or the query names no
    /// folder.
    pub fn selected(&self) -> Option<PathBuf> {
        let Listing::Ready(loaded) = self.listing()? else {
            return None;
        };
        let dir = directory_path(&self.dir_text, self.windows);
        if self.query.is_empty() {
            Some(dir)
        } else if loaded.names.contains(self.query.as_ref() as &str) {
            Some(dir.join(self.query.as_ref()))
        } else {
            None
        }
    }

    /// Chooses [`selected`](Self::selected), if there is one.
    pub fn choose(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.selected() {
            cx.emit(FolderPickerEvent::Chosen(path));
        }
    }

    /// Cancels the picker.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(FolderPickerEvent::Cancelled);
    }

    /// Goes to the parent of the listed directory. The root is its own
    /// parent.
    pub fn go_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = parent_text(&self.dir_text, self.windows);
        self.set_text(&text, window, cx);
    }

    /// Goes inside the folder `name` of the listed directory.
    pub fn descend(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = join_dir(&self.dir_text, name, self.windows);
        self.set_text(&text, window, cx);
    }

    /// Asks the source for the next page of the listed directory, when it
    /// has one and no request is running. After a failure it asks again.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir.clone();
        let Some(Listing::Ready(loaded)) = self.listings.get_mut(&dir) else {
            return;
        };
        let Some(token) = loaded.next.clone() else {
            return;
        };
        if loaded.more == MoreState::Loading {
            return;
        }
        loaded.more = MoreState::Loading;
        let id = self.load_id;
        cx.notify();
        self.load_more = Some(cx.spawn(async move |this, cx| {
            let Ok(answer) = this.update(cx, |this, cx| {
                this.source.clone().list(&dir, Some(token), cx)
            }) else {
                return;
            };
            let result = answer.await;
            this.update(cx, |this, cx| this.paged(dir, id, result, cx))
                .ok();
        }));
    }

    /// The list reached its last row: loads the next page unless a request
    /// is running or the last one failed, which the row retries on a
    /// click.
    fn load_more_on_scroll(&mut self, cx: &mut Context<Self>) {
        if let Some(Listing::Ready(loaded)) = self.listing()
            && loaded.more == MoreState::Idle
        {
            self.load_more(cx);
        }
    }

    /// Replaces the path text from code, which the field does not report.
    fn set_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx)
        });
        self.apply_text(text.to_string(), cx);
    }

    /// The user edited the path text.
    fn text_changed(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let typed_separator = !self.query.is_empty()
            && text
                == format!(
                    "{}{}{}",
                    self.dir_text,
                    self.query,
                    path::separator(self.windows)
                )
            || (!self.query.is_empty()
                && self.windows
                && text == format!("{}{}/", self.dir_text, self.query));
        if !typed_separator {
            self.pending = None;
            self.apply_text(text, cx);
            return;
        }
        if self.filter.is_some() {
            // The rows still answer an older query. Wait for the filter of
            // this one before choosing a folder.
            self.pending = Some(PendingDescend { text });
        } else {
            self.descend_or_keep(text, window, cx);
        }
    }

    /// A separator was typed after the query: goes inside the folder the
    /// query resolves to, or keeps the literal text.
    fn descend_or_keep(&mut self, typed: String, window: &mut Window, cx: &mut Context<Self>) {
        let name = match self.listing() {
            Some(Listing::Ready(loaded)) => {
                let matches = match &self.shown {
                    Shown::Ranked(matches) => matches.as_slice(),
                    Shown::All(_) => &[],
                };
                resolve_descend(loaded.entries(), matches, &self.query)
            }
            _ => None,
        };
        match name {
            Some(name) => self.descend(&name, window, cx),
            None => self.apply_text(typed, cx),
        }
    }

    /// Splits `text` into the directory and the query, lists the
    /// directory when it changed, and filters its folders.
    fn apply_text(&mut self, text: String, cx: &mut Context<Self>) {
        let (dir_text, query) = split_path(&text, self.windows);
        let dir = directory_path(&dir_text, self.windows);
        self.dir_text = dir_text;
        self.query = query.into();
        if dir != self.dir {
            self.enter(dir, cx);
        }
        self.refilter(false, cx);
    }

    /// Moves to `dir`: cancels the requests for the directory left, keeps
    /// only the listings that finished, and lists `dir` unless it is
    /// cached.
    fn enter(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.load = None;
        self.load_more = None;
        self.load_id += 1;
        if let Some(Listing::Ready(loaded)) = self.listings.get_mut(&self.dir)
            && loaded.more == MoreState::Loading
        {
            loaded.more = MoreState::Idle;
        }
        self.listings
            .retain(|_, listing| matches!(listing, Listing::Ready(_)));
        self.dir = dir.clone();
        if self.listings.contains_key(&dir) {
            return;
        }
        self.listings.insert(dir.clone(), Listing::Loading);
        let id = self.load_id;
        // The source is asked once the caller's builder chain has ended, so
        // a picker that is seeded and given a source asks for the last
        // directory only.
        self.load = Some(cx.spawn(async move |this, cx| {
            let Ok(answer) = this.update(cx, |this, cx| this.source.clone().list(&dir, None, cx))
            else {
                return;
            };
            let result = answer.await;
            this.update(cx, |this, cx| this.loaded(dir, id, result, cx))
                .ok();
        }));
    }

    /// The first page arrived. It is dropped when the path has moved on.
    fn loaded(
        &mut self,
        dir: PathBuf,
        id: u64,
        result: io::Result<FolderPage>,
        cx: &mut Context<Self>,
    ) {
        if id != self.load_id || dir != self.dir {
            return;
        }
        self.load = None;
        let listing = match result {
            Ok(page) => Listing::Ready(Loaded::from_page(page)),
            Err(_) => Listing::Failed,
        };
        self.listings.insert(dir, listing);
        self.refilter(false, cx);
    }

    /// A later page arrived. It is dropped when the path has moved on.
    fn paged(
        &mut self,
        dir: PathBuf,
        id: u64,
        result: io::Result<FolderPage>,
        cx: &mut Context<Self>,
    ) {
        if id != self.load_id || dir != self.dir {
            return;
        }
        self.load_more = None;
        let Some(Listing::Ready(loaded)) = self.listings.get_mut(&dir) else {
            return;
        };
        match result {
            Ok(page) => loaded.append(page),
            Err(_) => loaded.more = MoreState::Failed,
        }
        self.refilter(true, cx);
    }

    /// Filters the listed folders for the query: at once for an empty
    /// query, and on a background task for any other, which the next call
    /// cancels. `keep_place` keeps the scroll position and the highlight,
    /// for a page appended to the list.
    fn refilter(&mut self, keep_place: bool, cx: &mut Context<Self>) {
        self.filter = None;
        self.filter_id += 1;
        let id = self.filter_id;
        let Some(Listing::Ready(loaded)) = self.listing() else {
            self.show(Shown::All(0), false, cx);
            return;
        };
        if self.query.is_empty() {
            let shown = Shown::All(loaded.entries.len());
            self.show(shown, keep_place, cx);
            return;
        }
        let entries = loaded.entries.clone();
        let query = self.query.to_string();
        let window = self.window;
        self.filter = Some(cx.spawn(async move |this, cx| {
            let matches = cx
                .background_spawn(async move { filter(&entries, &query) })
                .await;
            let pending = this
                .update(cx, |this, cx| {
                    if this.filter_id != id {
                        return None;
                    }
                    this.filter = None;
                    this.show(Shown::Ranked(matches), keep_place, cx);
                    this.pending.take()
                })
                .ok()
                .flatten();
            if let Some(pending) = pending {
                // A separator was typed while the filter ran, and the
                // field needs the window to take the folder's name.
                cx.update_window(window, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        this.descend_or_keep(pending.text, window, cx)
                    })
                    .ok();
                })
                .ok();
            }
        }));
        cx.notify();
    }

    /// Whether the list ends with a "Load more" row.
    fn has_more_row(&self) -> bool {
        matches!(self.listing(), Some(Listing::Ready(loaded)) if loaded.next.is_some())
    }

    /// The rows of the list: the folders and the "Load more" row.
    fn item_count(&self) -> usize {
        self.shown.len() + usize::from(self.has_more_row())
    }

    fn show(&mut self, shown: Shown, keep_place: bool, cx: &mut Context<Self>) {
        self.shown = shown;
        let count = self.item_count();
        if keep_place {
            self.list.splice(0..self.list_count, count);
            self.highlighted = self.highlighted.filter(|row| *row < count);
        } else {
            self.list.reset(count);
            self.highlighted = (self.shown.len() > 0).then_some(0);
        }
        self.list_count = count;
        cx.notify();
    }

    /// What the list area shows.
    fn body(&self) -> Body {
        match self.listing() {
            None | Some(Listing::Loading) => Body::Loading,
            Some(Listing::Failed) => Body::Failed,
            Some(Listing::Ready(loaded)) => {
                if self.has_more_row() {
                    Body::List
                } else if loaded.entries.is_empty() {
                    Body::Empty
                } else if self.shown.len() == 0 {
                    Body::NoMatch
                } else {
                    Body::List
                }
            }
        }
    }

    /// How the request for the next page stands.
    fn more_state(&self) -> MoreState {
        match self.listing() {
            Some(Listing::Ready(loaded)) => loaded.more,
            _ => MoreState::Idle,
        }
    }

    /// The folder shown at `row`, and where the query matched its name.
    fn entry_at(&self, row: usize) -> Option<(&FolderEntry, &[std::ops::Range<usize>])> {
        let Some(Listing::Ready(loaded)) = self.listing() else {
            return None;
        };
        match &self.shown {
            Shown::All(_) => loaded.entries.get(row).map(|entry| (entry, &[][..])),
            Shown::Ranked(matches) => {
                let found = matches.get(row)?;
                Some((loaded.entries.get(found.index)?, found.ranges.as_slice()))
            }
        }
    }

    /// Moves the highlight one row up or down, wrapping at the ends.
    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.item_count() as isize;
        if count == 0 {
            return;
        }
        let row = match self.highlighted {
            Some(row) => (row as isize + delta).rem_euclid(count),
            None if delta > 0 => 0,
            None => count - 1,
        } as usize;
        self.highlighted = Some(row);
        self.list.scroll_to_reveal_item(row);
        cx.notify();
    }

    /// Enter: goes inside the highlighted folder, or loads more when the
    /// highlight is on the "Load more" row.
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.highlighted {
            self.enter_row(row, window, cx);
        }
    }

    /// Goes inside the folder at `row`, or loads more for the row after
    /// the last folder.
    fn enter_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        if row == self.shown.len() && self.has_more_row() {
            self.load_more(cx);
        } else if let Some(name) = self.entry_at(row).map(|(entry, _)| entry.name().clone()) {
            self.descend(&name, window, cx);
        }
    }
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type OpenChange = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A dialog to choose a folder on a [`FolderPickerState`], as in the
/// reference app: an Up button and the path field, a row that chooses
/// the folder shown, a bordered list of its folders, and Cancel and
/// "Use folder" in the footer.
///
/// ```
/// use gpui_cn::{FolderPicker, FolderPickerState};
/// use gpui_kit::{Entity, IntoElement};
///
/// fn picker(state: &Entity<FolderPickerState>, open: bool) -> impl IntoElement {
///     FolderPicker::new("source-folder", state)
///         .open(open)
///         .title("Choose a source folder")
/// }
/// ```
///
/// The application owns the open state: [`open`](Self::open) sets it, and
/// it closes the picker when the state reports
/// [`FolderPickerEvent::Chosen`] or [`FolderPickerEvent::Cancelled`].
/// The id must be stable across frames.
#[derive(IntoElement)]
pub struct FolderPicker {
    id: ElementId,
    state: Entity<FolderPickerState>,
    open: bool,
    title: SharedString,
    on_open_change: Option<OpenChange>,
}

impl FolderPicker {
    /// A closed picker on `state`, with a stable id.
    pub fn new(id: impl Into<ElementId>, state: &Entity<FolderPickerState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            open: false,
            title: DEFAULT_TITLE.into(),
            on_open_change: None,
        }
    }

    /// Shows or hides the picker.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The title. The default is "Choose a folder".
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Called when Escape, a press on the backdrop, the close button, or
    /// Cancel asks to close the picker, after the state reports
    /// [`FolderPickerEvent::Cancelled`].
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FolderPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let look = MenuLook::of(theme, window.rem_size());
        let list_height = theme.metrics.folder_list_height;
        let link = theme.link;
        let pointer_cursors = Theme::global(cx).pointer_cursors;
        let state = self.state;
        let child = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let (input, list, body_kind, selected, query_empty, has_entries) = {
            let state = state.read(cx);
            let has_entries = matches!(
                state.listing(),
                Some(Listing::Ready(loaded)) if !loaded.entries().is_empty()
            );
            (
                state.input.clone(),
                state.list.clone(),
                state.body(),
                state.selected(),
                state.query.is_empty(),
                has_entries,
            )
        };
        let focus = input.read(cx).focus_handle(cx);

        let close: CloseHandler = {
            let state = state.clone();
            let handler = self.on_open_change;
            Rc::new(move |window, cx| {
                state.update(cx, |state, cx| state.cancel(cx));
                if let Some(handler) = &handler {
                    handler(false, window, cx);
                }
            })
        };

        let message = |text: &'static str| {
            div()
                .id(child("message"))
                .test_support()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(look.muted_foreground)
                .child(text)
                .into_any_element()
        };
        let body = match body_kind {
            Body::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(look.muted_foreground)
                .child(Spinner::new(child("loading")))
                .into_any_element(),
            Body::Failed => message(FAILED),
            Body::Empty => message(EMPTY),
            Body::NoMatch => message(NO_MATCH),
            Body::List => {
                let rows = Rows {
                    id: self.id.clone(),
                    state: state.clone(),
                    look: look.clone(),
                    link,
                    pointer_cursors,
                };
                ScrollArea::list(
                    child("rows"),
                    &list,
                    look.row_height,
                    move |row, window, cx| rows.render(row, window, cx),
                )
                .size_full()
                .into_any_element()
            }
        };

        let up = {
            let state = state.clone();
            Button::new(child("up"))
                .ghost()
                .icon(Icon::from(IconName::ArrowUp))
                .accessibility_label("Parent folder")
                .on_click(move |_, window, cx| {
                    state.update(cx, |state, cx| state.go_up(window, cx));
                })
        };
        let current = (query_empty && has_entries).then(|| {
            let state = state.clone();
            h_flex()
                .id(child("current"))
                .test_support()
                .role(Role::Button)
                .h(look.row_height)
                .px(look.row_padding)
                .items_center()
                .text_color(look.muted_foreground)
                .when(pointer_cursors, |this| this.cursor_pointer())
                .child("Select current folder")
                .on_click(move |_, _, cx| state.update(cx, |state, cx| state.choose(cx)))
        });
        let (up_key, down_key, confirm_key) = (state.clone(), state.clone(), state.clone());
        let content = v_flex()
            .key_context(CONTEXT)
            .gap_2()
            .on_action(move |_: &SelectUp, _, cx| {
                up_key.update(cx, |state, cx| state.move_highlight(-1, cx));
            })
            .on_action(move |_: &SelectDown, _, cx| {
                down_key.update(cx, |state, cx| state.move_highlight(1, cx));
            })
            .on_action({
                let state = state.clone();
                move |_: &Submit, _, cx| state.update(cx, |state, cx| state.choose(cx))
            })
            .on_action(move |_: &Confirm, window, cx| {
                confirm_key.update(cx, |state, cx| state.confirm(window, cx));
            })
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(up)
                    .child(div().flex_1().child(crate::Input::new(&input))),
            )
            .children(current)
            .child(
                div()
                    .id(child("list"))
                    .test_support()
                    .h(list_height + look.padding * 2. + gpui_kit::px(2.))
                    .p(look.padding)
                    .rounded(look.radius)
                    .border_1()
                    .border_color(look.border)
                    .overflow_hidden()
                    .child(body),
            );

        let footer = h_flex()
            .gap_2()
            .child({
                let close = close.clone();
                Button::new(child("cancel"))
                    .ghost()
                    .label("Cancel")
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .child({
                let state = state.clone();
                Button::new(child("use"))
                    .primary()
                    .label("Use folder")
                    .disabled(selected.is_none())
                    .on_click(move |_, _, cx| state.update(cx, |state, cx| state.choose(cx)))
            });

        Dialog::new(self.id.clone())
            .open(self.open)
            .title(self.title)
            .track_focus(&focus)
            .on_open_change(move |_, window, cx| close(window, cx))
            .child(content)
            .footer(footer)
    }
}

/// What a row is drawn from, owned by the list's row builder, which
/// outlives the render that made it.
struct Rows {
    id: ElementId,
    state: Entity<FolderPickerState>,
    look: MenuLook,
    link: gpui_kit::Hsla,
    pointer_cursors: bool,
}

impl Rows {
    fn render(&self, row: usize, _: &mut Window, cx: &mut App) -> AnyElement {
        let look = &self.look;
        let (name, ranges, highlighted) = {
            let state = self.state.read(cx);
            if row == state.shown.len() && state.has_more_row() {
                return self.more_row(state.more_state(), state.highlighted == Some(row), row);
            }
            let Some((entry, ranges)) = state.entry_at(row) else {
                return div().into_any_element();
            };
            (
                entry.name().clone(),
                ranges.to_vec(),
                state.highlighted == Some(row),
            )
        };
        let label =
            StyledText::new(name.clone()).with_highlights(ranges.into_iter().map(|range| {
                (
                    range,
                    HighlightStyle {
                        color: Some(self.link),
                        font_weight: Some(FontWeight::SEMIBOLD),
                        ..Default::default()
                    },
                )
            }));
        let state = self.state.clone();
        let hover = self.state.clone();
        h_flex()
            .id(ElementId::NamedChild(self.id.clone().into(), name))
            .test_support()
            .role(Role::ListBoxOption)
            .w_full()
            .h(look.row_height)
            .px(look.row_padding)
            .gap_2()
            .items_center()
            .rounded(look.row_radius)
            .when(highlighted, |this| {
                this.aria_active_descendant().bg(look.accent)
            })
            .when(self.pointer_cursors, |this| this.cursor_pointer())
            .child(
                Icon::from(IconName::Folder)
                    .size_4()
                    .flex_shrink_0()
                    .text_color(look.muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(label),
            )
            .when(!look.touch, |this| {
                this.on_mouse_move(move |_, _, cx| {
                    hover.update(cx, |state, cx| {
                        if state.highlighted != Some(row) {
                            state.highlighted = Some(row);
                            cx.notify();
                        }
                    });
                })
            })
            .on_click(move |_, window, cx| {
                state.update(cx, |state, cx| state.enter_row(row, window, cx));
            })
            .into_any_element()
    }

    /// The last row while more folders follow: it loads them on a click,
    /// and asks again after a failure.
    fn more_row(&self, more: MoreState, highlighted: bool, row: usize) -> AnyElement {
        let look = &self.look;
        let state = self.state.clone();
        let hover = self.state.clone();
        h_flex()
            .id(ElementId::NamedChild(self.id.clone().into(), "more".into()))
            .test_support()
            .role(Role::Button)
            .w_full()
            .h(look.row_height)
            .px(look.row_padding)
            .gap_2()
            .items_center()
            .rounded(look.row_radius)
            .text_color(look.muted_foreground)
            .when(highlighted, |this| {
                this.aria_active_descendant().bg(look.accent)
            })
            .when(self.pointer_cursors, |this| this.cursor_pointer())
            .when(more == MoreState::Loading, |this| {
                this.child(Spinner::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    "more-spinner".into(),
                )))
            })
            .child(match more {
                MoreState::Idle => LOAD_MORE,
                MoreState::Loading => LOADING_MORE,
                MoreState::Failed => MORE_FAILED,
            })
            .when(!look.touch, |this| {
                this.on_mouse_move(move |_, _, cx| {
                    hover.update(cx, |state, cx| {
                        if state.highlighted != Some(row) {
                            state.highlighted = Some(row);
                            cx.notify();
                        }
                    });
                })
            })
            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.load_more(cx)))
            .into_any_element()
    }
}
