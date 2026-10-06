//! What the folder picker and the file picker share: the path text and its
//! `~`, the listings and their pages, the filter, the highlight, and the
//! list. A picker owns a [`Browser`] and is its [`Host`]; the browser does
//! the work and the host decides what a row means.

pub(crate) mod path;
mod style;
pub(crate) mod view;

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, KeyBinding, ListAlignment, ListState,
    SharedString, Subscription, Task, Window, actions,
    base::{
        actions::{Confirm, SelectDown, SelectUp},
        input::{InputEvent, InputState},
    },
};

use crate::ActiveTheme as _;
pub(crate) use path::Match;
use path::{
    all_visible, collapse_home, is_home_text, join_dir, parent_text, rank, resolve_descend,
    resolve_dir, split_path,
};
pub use style::{ListError, PathStyle, SourcePath};

actions!(
    gpui_cn_path_picker,
    [
        /// Chooses what the picker shows, as its confirm button does.
        Submit
    ]
);

/// The shortcut that submits: Cmd+Enter on macOS, Ctrl+Enter elsewhere.
const SUBMIT_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-enter"
} else {
    "ctrl-enter"
};

/// Binds the keys a picker's dialog takes, in its key `context`: Up and
/// Down move the highlight, Enter confirms the row, and the submit
/// shortcut chooses.
pub(crate) fn bind_keys(context: &'static str, cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(SUBMIT_KEY, Submit, Some(context)),
        KeyBinding::new("up", SelectUp, Some(context)),
        KeyBinding::new("down", SelectDown, Some(context)),
        KeyBinding::new("enter", Confirm { secondary: false }, Some(context)),
    ]);
}

/// An entry of a listing, as the browser needs to know it. It is public
/// only so the listing types can name it; applications implement their own
/// entry types, not this trait.
pub trait Entry: Clone + Send + Sync + 'static {
    /// The name, without a path.
    fn name(&self) -> &SharedString;
    /// Whether the entry is hidden.
    fn hidden(&self) -> bool;
    /// Whether the entry is a folder, which a picker can go inside.
    fn is_folder(&self) -> bool;
}

/// Where the next page of a listing starts. The source makes it and reads
/// it back, so it can hold anything: an offset, a cursor, a continuation
/// token. The picker only stores it and hands it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageToken(Arc<str>);

impl PageToken {
    /// A token that holds `value`.
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    /// The value the source put in the token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One page of a listing.
pub(crate) struct Page<E> {
    pub(crate) entries: Vec<E>,
    pub(crate) next: Option<PageToken>,
}

/// What a browser lists from.
pub(crate) trait Source<E>: 'static {
    fn list(
        &self,
        dir: &SourcePath,
        page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<Result<Page<E>, ListError>>;

    fn home(&self) -> Option<SourcePath>;

    fn style(&self) -> PathStyle;

    fn env(&self, name: &str) -> Option<String>;
}

/// Where the request for the next page of a listing stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MoreState {
    /// No request is running. Reaching the end of the list starts one.
    Idle,
    /// The next page is on its way.
    Loading,
    /// The last request failed. The entries loaded stay, and the "Load
    /// more" row asks again.
    Failed,
}

/// The entries of a directory loaded so far, and whether more follow.
#[derive(Debug)]
#[non_exhaustive]
pub struct Loaded<E> {
    pub(crate) entries: Arc<Vec<E>>,
    style: PathStyle,
    names: HashSet<String>,
    pub(crate) next: Option<PageToken>,
    pub(crate) more: MoreState,
}

impl<E: Entry> Loaded<E> {
    fn from_page(page: Page<E>, style: PathStyle) -> Self {
        let mut loaded = Self {
            entries: Arc::new(Vec::new()),
            style,
            names: HashSet::new(),
            next: None,
            more: MoreState::Idle,
        };
        loaded.append(page);
        loaded
    }

    /// Adds a page: its entries, less the names already loaded, and where
    /// the following page starts.
    fn append(&mut self, page: Page<E>) {
        self.next = page.next;
        let entries = Arc::make_mut(&mut self.entries);
        for entry in page.entries {
            if self
                .names
                .insert(self.style.fold(entry.name()).into_owned())
            {
                entries.push(entry);
            }
        }
        self.more = MoreState::Idle;
    }
}

impl<E: Entry> Loaded<E> {
    /// The entry named `name`, by the source's own rule for case.
    pub(crate) fn entry_named(&self, name: &str) -> Option<&E> {
        let wanted = self.style.fold(name);
        self.entries
            .iter()
            .find(|entry| self.style.fold(entry.name()) == wanted)
    }
}

impl<E> Loaded<E> {
    /// The entries loaded, in the order the source gave them.
    pub fn entries(&self) -> &[E] {
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

/// What a browser knows about one directory.
#[derive(Debug)]
#[non_exhaustive]
pub enum Listing<E> {
    /// The source has not answered for the first page yet.
    Loading,
    /// The entries loaded so far.
    Ready(Loaded<E>),
    /// The source failed for the first page, such as for a path that does
    /// not exist, with why.
    Failed(ListError),
}

/// What the list area shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    Loading,
    Failed,
    Empty,
    NoMatch,
    List,
}

/// How many characters `new` has that `old` does not, counting what lies
/// between their common start and common end: a keystroke inserts one, a
/// paste more, and a deletion none.
fn inserted_chars(old: &str, new: &str) -> usize {
    let start = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let end = old
        .chars()
        .rev()
        .zip(new.chars().rev())
        .take(old.chars().count().min(new.chars().count()) - start)
        .take_while(|(a, b)| a == b)
        .count();
    new.chars().count() - start - end
}

/// A separator typed after a query, waiting for the filter of that query
/// to finish so it knows which folder to go into.
struct PendingDescend {
    text: String,
}

type Visible<E> = Arc<dyn Fn(&E) -> bool + Send + Sync>;

/// What the application does when the user asks to sign in to a source.
pub(crate) type AuthHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The owner of a browser: it gives access to it, and the browser calls
/// back through the owner's context.
pub(crate) trait Host<E: Entry>: 'static + Sized {
    fn browser(&self) -> &Browser<E>;
    fn browser_mut(&mut self) -> &mut Browser<E>;
}

/// The path text, the listings, and the filtered rows.
///
/// The text is a path. Everything up to and including its last separator
/// is the directory that is listed; what follows is a fuzzy query over
/// that directory's entries. A leading `~` stands for the source's home.
/// Every listing is cached, so going back is instant. An answer for a
/// directory the path has left is dropped, and a new query cancels the
/// filter before it.
pub(crate) struct Browser<E: Entry> {
    pub(crate) input: Entity<InputState>,
    source: Rc<dyn Source<E>>,
    pub(crate) style: PathStyle,
    window: AnyWindowHandle,
    pub(crate) dir_text: String,
    /// The text of the field after the last change the browser took, to
    /// tell a paste from a keystroke.
    last_text: String,
    pub(crate) dir: SourcePath,
    pub(crate) query: SharedString,
    pub(crate) listings: HashMap<SourcePath, Listing<E>>,
    /// The running listing of the first page. Dropping it cancels it.
    load: Option<Task<()>>,
    /// The running request for a later page. Dropping it cancels it.
    load_more: Option<Task<()>>,
    /// Which directory visit the running requests belong to.
    load_id: u64,
    pub(crate) shown: Vec<Match>,
    pub(crate) highlighted: Option<usize>,
    pub(crate) list: ListState,
    /// How many items the list was told it has.
    list_count: usize,
    /// The running filter. Dropping it cancels it.
    filter: Option<Task<()>>,
    filter_id: u64,
    pending: Option<PendingDescend>,
    visible: Visible<E>,
    /// Whether the caller named the start folder, which wins over the
    /// source's home.
    pub(crate) explicit_start: bool,
    auth_handler: Option<AuthHandler>,
}

impl<E: Entry> Browser<E> {
    /// A browser with an empty path field. The host calls
    /// [`start`](Self::start) once it holds the browser.
    pub(crate) fn new<O: Host<E>>(
        source: Rc<dyn Source<E>>,
        window: &mut Window,
        cx: &mut Context<O>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Path"));
        let overdraw = cx.theme().metrics.folder_list_height;
        let list = ListState::new(0, ListAlignment::Top, overdraw);
        let weak = cx.weak_entity();
        list.set_scroll_handler(move |event, _, cx| {
            if event.count > 0 && event.visible_range.end >= event.count {
                weak.update(cx, |owner: &mut O, cx| {
                    owner.browser_mut().load_more_on_scroll(cx)
                })
                .ok();
            }
        });
        let style = source.style();
        Self {
            input,
            source,
            style,
            window: window.window_handle(),
            dir_text: String::new(),
            last_text: String::new(),
            dir: style.path(""),
            query: SharedString::default(),
            listings: HashMap::new(),
            load: None,
            load_more: None,
            load_id: 0,
            shown: Vec::new(),
            highlighted: None,
            list,
            list_count: 0,
            filter: None,
            filter_id: 0,
            pending: None,
            visible: Arc::new(|_| true),
            explicit_start: false,
            auth_handler: None,
        }
    }

    /// Sets what the "Sign in" button of a source that needs a login does.
    pub(crate) fn set_auth_handler(&mut self, handler: AuthHandler) {
        self.auth_handler = Some(handler);
    }

    pub(crate) fn auth_handler(&self) -> Option<AuthHandler> {
        self.auth_handler.clone()
    }

    /// Why the listed directory failed, when it did.
    pub(crate) fn failure(&self) -> Option<&ListError> {
        match self.listing()? {
            Listing::Failed(error) => Some(error),
            _ => None,
        }
    }

    /// Lists the directory again: its answer is dropped, the spinner shows,
    /// and the source is asked for the first page.
    pub(crate) fn retry<O: Host<E>>(&mut self, cx: &mut Context<O>) {
        let dir = self.dir.clone();
        self.listings.remove(&dir);
        self.enter(dir, cx);
        self.refilter(false, cx);
    }

    /// Cancels every request in flight: a running listing is dropped, so
    /// its answer never arrives, and a directory that was still loading
    /// is forgotten so it is listed again when the picker shows.
    pub(crate) fn stop_requests<O: Host<E>>(&mut self, cx: &mut Context<O>) {
        self.load = None;
        self.load_more = None;
        self.filter = None;
        self.filter_id += 1;
        self.pending = None;
        self.load_id += 1;
        match self.listings.get_mut(&self.dir) {
            Some(Listing::Loading) => {
                self.listings.remove(&self.dir);
            }
            Some(Listing::Ready(loaded)) if loaded.more == MoreState::Loading => {
                loaded.more = MoreState::Idle;
            }
            _ => {}
        }
        cx.notify();
    }

    /// Lists the directory when no listing of it is known or running, as
    /// after [`stop_requests`](Self::stop_requests).
    pub(crate) fn ensure_listed<O: Host<E>>(&mut self, cx: &mut Context<O>) {
        if self.listings.contains_key(&self.dir) || self.dir_text.is_empty() {
            return;
        }
        let dir = self.dir.clone();
        self.enter(dir, cx);
        self.refilter(false, cx);
    }

    /// The subscription that feeds the path field's edits to the browser.
    pub(crate) fn watch<O: Host<E>>(
        &self,
        window: &mut Window,
        cx: &mut Context<O>,
    ) -> Subscription {
        cx.subscribe_in(
            &self.input,
            window,
            |this: &mut O, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    this.browser_mut().text_changed(text, window, cx);
                }
            },
        )
    }

    /// The text the browser starts with when it was given no start: the
    /// source's home, or the root.
    pub(crate) fn default_start(&self) -> String {
        if self.source.home().is_some() {
            format!("~{}", self.style.separator())
        } else {
            self.style.separator().to_string()
        }
    }

    /// The entries that show are those `visible` lets through. A folder
    /// picker lets every entry through; a file picker lets folders and the
    /// files its filter accepts.
    pub(crate) fn set_visible<O: Host<E>>(
        &mut self,
        visible: impl Fn(&E) -> bool + Send + Sync + 'static,
        cx: &mut Context<O>,
    ) {
        self.visible = Arc::new(visible);
        self.refilter(false, cx);
    }

    /// Lists from `source`: the cached listings are dropped, and the
    /// browser starts at the new source's home unless the caller named a
    /// start.
    pub(crate) fn set_source<O: Host<E>>(
        &mut self,
        source: Rc<dyn Source<E>>,
        window: &mut Window,
        cx: &mut Context<O>,
    ) {
        self.style = source.style();
        self.source = source;
        self.listings.clear();
        self.load = None;
        self.load_more = None;
        self.dir = self.style.path("");
        let text = if self.explicit_start {
            self.input.read(cx).value().to_string()
        } else {
            self.default_start()
        };
        self.set_text(&text, window, cx);
    }

    /// Starts in `path`, which is listed at once.
    pub(crate) fn start_in<O: Host<E>>(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<O>,
    ) {
        self.explicit_start = true;
        let mut text = path.to_string();
        if !text.ends_with(|c| self.style.is_separator(c)) {
            text.push(self.style.separator());
        }
        self.set_text(&text, window, cx);
    }

    /// Which visit to a directory the browser is in: it changes every
    /// time the browser moves to another directory, and when it comes back.
    pub(crate) fn visit(&self) -> u64 {
        self.load_id
    }

    /// What is known about the directory that is listed.
    pub(crate) fn listing(&self) -> Option<&Listing<E>> {
        self.listings.get(&self.dir)
    }

    /// The loaded entries of the listed directory.
    pub(crate) fn loaded(&self) -> Option<&Loaded<E>> {
        match self.listing()? {
            Listing::Ready(loaded) => Some(loaded),
            _ => None,
        }
    }

    /// How many entries the list shows, not counting the "Load more" row.
    pub(crate) fn row_count(&self) -> usize {
        self.shown.len()
    }

    /// Whether the list ends with a "Load more" row.
    pub(crate) fn has_more_row(&self) -> bool {
        matches!(self.listing(), Some(Listing::Ready(loaded)) if loaded.next.is_some())
    }

    /// The rows of the list: the entries and the "Load more" row.
    pub(crate) fn item_count(&self) -> usize {
        self.shown.len() + usize::from(self.has_more_row())
    }

    /// What the list area shows.
    pub(crate) fn body(&self) -> Body {
        match self.listing() {
            None | Some(Listing::Loading) => Body::Loading,
            Some(Listing::Failed(_)) => Body::Failed,
            Some(Listing::Ready(_)) => {
                if self.has_more_row() || !self.shown.is_empty() {
                    Body::List
                } else if self.query.is_empty() {
                    Body::Empty
                } else {
                    Body::NoMatch
                }
            }
        }
    }

    /// How the request for the next page stands.
    pub(crate) fn more_state(&self) -> MoreState {
        match self.listing() {
            Some(Listing::Ready(loaded)) => loaded.more,
            _ => MoreState::Idle,
        }
    }

    /// The entry shown at `row`, and where the query matched its name.
    pub(crate) fn entry_at(&self, row: usize) -> Option<(&E, &[std::ops::Range<usize>])> {
        let loaded = self.loaded()?;
        let found = self.shown.get(row)?;
        Some((loaded.entries.get(found.index)?, found.ranges.as_slice()))
    }

    /// Replaces the path text from code, which the field does not report.
    pub(crate) fn set_text<O: Host<E>>(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<O>,
    ) {
        self.input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx)
        });
        self.last_text = text.to_string();
        self.apply_text(text.to_string(), cx);
    }

    /// Goes to the parent of the listed directory. The root is its own
    /// parent, and the home's parent is the folder above it.
    pub(crate) fn go_up<O: Host<E>>(&mut self, window: &mut Window, cx: &mut Context<O>) {
        let home = self.source.home();
        let at_home = home.is_some()
            && is_home_text(&self.dir_text, &self.style)
            && self
                .dir_text
                .trim_end_matches(|c| self.style.is_separator(c))
                == "~";
        let text = if at_home {
            let home = home
                .clone()
                .map(|home| home.to_string())
                .unwrap_or_default();
            parent_text(&home, &self.style)
        } else {
            parent_text(&self.dir_text, &self.style)
        };
        let text = collapse_home(&text, home.as_ref(), &self.style);
        self.set_text(&text, window, cx);
    }

    /// Goes inside the folder `name` of the listed directory.
    pub(crate) fn descend<O: Host<E>>(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<O>,
    ) {
        let text = join_dir(&self.dir_text, name, &self.style);
        let text = collapse_home(&text, self.source.home().as_ref(), &self.style);
        self.set_text(&text, window, cx);
    }

    /// Asks the source for the next page of the listed directory, when it
    /// has one and no request is running. After a failure it asks again.
    pub(crate) fn load_more<O: Host<E>>(&mut self, cx: &mut Context<O>) {
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
            let Ok(answer) = this.update(cx, |this: &mut O, cx| {
                let source = this.browser().source.clone();
                source.list(&dir, Some(token), cx)
            }) else {
                return;
            };
            let result = answer.await;
            this.update(cx, |this: &mut O, cx| {
                this.browser_mut().paged(dir, id, result, cx)
            })
            .ok();
        }));
    }

    /// The list reached its last row: loads the next page unless a request
    /// is running or the last one failed, which the row retries on a
    /// click.
    fn load_more_on_scroll<O: Host<E>>(&mut self, cx: &mut Context<O>) {
        if let Some(Listing::Ready(loaded)) = self.listing()
            && loaded.more == MoreState::Idle
        {
            self.load_more(cx);
        }
    }

    /// The user edited the path text.
    fn text_changed<O: Host<E>>(&mut self, text: String, window: &mut Window, cx: &mut Context<O>) {
        if inserted_chars(&self.last_text, &text) > 1 {
            let source = self.source.clone();
            let env = move |name: &str| source.env(name);
            let clean = self.style.normalize_pasted(&text, &env);
            if clean != text {
                self.set_text(&clean, window, cx);
                return;
            }
        }
        self.last_text = text.clone();
        let typed_separator = !self.query.is_empty()
            && self
                .style
                .is_separator(text.chars().last().unwrap_or_default())
            && text[..text.len() - 1] == format!("{}{}", self.dir_text, self.query);
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
    fn descend_or_keep<O: Host<E>>(
        &mut self,
        typed: String,
        window: &mut Window,
        cx: &mut Context<O>,
    ) {
        let name = self.loaded().and_then(|loaded| {
            resolve_descend(loaded.entries(), &self.shown, &self.query, &self.style)
        });
        match name {
            Some(name) => self.descend(&name, window, cx),
            None => self.apply_text(typed, cx),
        }
    }

    /// Splits `text` into the directory and the query, lists the
    /// directory when it changed, and filters its entries.
    fn apply_text<O: Host<E>>(&mut self, text: String, cx: &mut Context<O>) {
        let (dir_text, query) = if text == "~" && self.source.home().is_some() {
            (format!("~{}", self.style.separator()), String::new())
        } else {
            split_path(&text, &self.style)
        };
        let dir = resolve_dir(&dir_text, self.source.home().as_ref(), &self.style);
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
    fn enter<O: Host<E>>(&mut self, dir: SourcePath, cx: &mut Context<O>) {
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
            let Ok(answer) = this.update(cx, |this: &mut O, cx| {
                let source = this.browser().source.clone();
                source.list(&dir, None, cx)
            }) else {
                return;
            };
            let result = answer.await;
            this.update(cx, |this: &mut O, cx| {
                this.browser_mut().first_page(dir, id, result, cx)
            })
            .ok();
        }));
    }

    /// The first page arrived. It is dropped when the path has moved on.
    fn first_page<O: Host<E>>(
        &mut self,
        dir: SourcePath,
        id: u64,
        result: Result<Page<E>, ListError>,
        cx: &mut Context<O>,
    ) {
        if id != self.load_id || dir != self.dir {
            return;
        }
        self.load = None;
        let listing = match result {
            Ok(page) => Listing::Ready(Loaded::from_page(page, self.style)),
            Err(error) => Listing::Failed(error),
        };
        self.listings.insert(dir, listing);
        self.refilter(false, cx);
    }

    /// A later page arrived. It is dropped when the path has moved on.
    fn paged<O: Host<E>>(
        &mut self,
        dir: SourcePath,
        id: u64,
        result: Result<Page<E>, ListError>,
        cx: &mut Context<O>,
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

    /// Filters the listed entries for the query: at once for an empty
    /// query, and on a background task for any other, which the next call
    /// cancels. `keep_place` keeps the scroll position and the highlight,
    /// for a page appended to the list.
    pub(crate) fn refilter<O: Host<E>>(&mut self, keep_place: bool, cx: &mut Context<O>) {
        self.filter = None;
        self.filter_id += 1;
        let id = self.filter_id;
        let Some(Listing::Ready(loaded)) = self.listing() else {
            self.show(Vec::new(), false, cx);
            return;
        };
        if self.query.is_empty() {
            let shown = all_visible(&loaded.entries, &*self.visible);
            self.show(shown, keep_place, cx);
            return;
        }
        let entries = loaded.entries.clone();
        let visible = self.visible.clone();
        let query = self.query.to_string();
        let window = self.window;
        self.filter = Some(cx.spawn(async move |this, cx| {
            let matches = cx
                .background_spawn(async move { rank(&entries, &*visible, &query) })
                .await;
            let pending = this
                .update(cx, |this: &mut O, cx| {
                    let browser = this.browser_mut();
                    if browser.filter_id != id {
                        return None;
                    }
                    browser.filter = None;
                    browser.show(matches, keep_place, cx);
                    browser.pending.take()
                })
                .ok()
                .flatten();
            if let Some(pending) = pending {
                // A separator was typed while the filter ran, and the
                // field needs the window to take the folder's name.
                cx.update_window(window, |_, window, cx| {
                    this.update(cx, |this: &mut O, cx| {
                        this.browser_mut().descend_or_keep(pending.text, window, cx)
                    })
                    .ok();
                })
                .ok();
            }
        }));
        cx.notify();
    }

    fn show<O: Host<E>>(&mut self, shown: Vec<Match>, keep_place: bool, cx: &mut Context<O>) {
        self.shown = shown;
        let count = self.item_count();
        if keep_place {
            self.list.splice(0..self.list_count, count);
            self.highlighted = self.highlighted.filter(|row| *row < count);
        } else {
            self.list.reset(count);
            self.highlighted = (!self.shown.is_empty()).then_some(0);
        }
        self.list_count = count;
        cx.notify();
    }

    /// Moves the highlight one row up or down, wrapping at the ends.
    pub(crate) fn move_highlight<O: Host<E>>(&mut self, delta: isize, cx: &mut Context<O>) {
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
}
