//! A dialog to choose a folder, on any file system: the path is typed
//! text, and the folders of its directory are listed, filtered fuzzily by
//! what follows the last separator.
//!
//! [`FolderPickerState`] holds the path, the listings, and the filtered
//! rows; [`FolderPicker`] draws them in a [`Dialog`]. Nothing waits on
//! the UI thread: a [`FolderSource`] returns a task for each page of a
//! listing, and the filter runs on a background task that the next
//! keystroke cancels.

mod source;

use std::rc::Rc;

use gpui_kit::{
    App, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName,
    base::{
        Disableable as _, TestSupportExt as _,
        actions::{Confirm, SelectDown, SelectUp},
        h_flex,
    },
    div,
    prelude::FluentBuilder as _,
};

pub use crate::path_browser::{MoreState, PageToken};
pub use source::{FolderEntry, FolderPage, FolderSource, LocalFolders};

use crate::{
    ActiveTheme as _, Button, Dialog, Icon, ScrollArea, Theme,
    menu::MenuLook,
    path_browser::{self, Body, Browser, Host, SourcePath, Submit, view},
};

/// The key context of the picker, which takes Up, Down, and Enter the
/// path field passes on.
const CONTEXT: &str = "GpuiCnFolderPicker";

const DEFAULT_TITLE: &str = "Choose a folder";

pub(crate) fn init(cx: &mut App) {
    path_browser::bind_keys(CONTEXT, cx);
}

/// The folders of a directory loaded so far, and whether more follow.
pub type Loaded = path_browser::Loaded<FolderEntry>;

/// What a [`FolderPickerState`] knows about one directory.
pub type Listing = path_browser::Listing<FolderEntry>;

/// What a [`FolderPickerState`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FolderPickerEvent {
    /// A folder was chosen. The path has no trailing separator, except
    /// for a root, and is in the style of the picker's source.
    Chosen(SourcePath),
    /// The picker was cancelled.
    Cancelled,
}

/// The path text, the listings, and the filtered rows of a
/// [`FolderPicker`].
///
/// The text is a path. Everything up to and including its last
/// separator is the directory that is listed; what follows is a fuzzy
/// query over that directory's folders. Typing a separator after a
/// query goes inside the folder named exactly like it, or else the best
/// match. Enter or a click on a row goes inside it too, and the Up
/// button goes to the parent. A leading `~` stands for the source's home.
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
    browser: Browser<FolderEntry>,
    _subscription: Subscription,
}

impl Host<FolderEntry> for FolderPickerState {
    fn browser(&self) -> &Browser<FolderEntry> {
        &self.browser
    }

    fn browser_mut(&mut self) -> &mut Browser<FolderEntry> {
        &mut self.browser
    }
}

impl EventEmitter<FolderPickerEvent> for FolderPickerState {}

/// The path field's focus, which holds the keyboard for the picker.
impl Focusable for FolderPickerState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.browser.input.read(cx).focus_handle(cx)
    }
}

impl FolderPickerState {
    /// A picker on the local file system, at the user's home folder (the
    /// root when there is none). Name another start with
    /// [`with_initial`](Self::with_initial).
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut browser = Browser::new(Rc::new(source::Adapter(Rc::new(LocalFolders))), window, cx);
        let subscription = browser.watch(window, cx);
        let start = browser.default_start();
        browser.set_text(&start, window, cx);
        Self {
            browser,
            _subscription: subscription,
        }
    }

    /// Lists folders from `source` instead of the local file system. The
    /// listings cached so far are dropped and the current directory is
    /// listed again. The picker starts at the new source's home, unless
    /// [`with_initial`](Self::with_initial) named a start, and at the root
    /// when the source has no home. A leading `~` in the path stands for
    /// the source's home. See [`FolderSource`] for how to write one.
    pub fn with_source(
        mut self,
        source: impl FolderSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.browser
            .set_source(Rc::new(source::Adapter(Rc::new(source))), window, cx);
        self
    }

    /// Starts in `path`, which is listed at once. A path that starts with
    /// `~` is in the source's home. This wins over the source's home as
    /// the start.
    pub fn with_initial(
        mut self,
        path: impl AsRef<str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.browser.start_in(path.as_ref(), window, cx);
        self
    }

    /// Sets what the "Sign in" button does. A source that needs the user to
    /// log in fails a listing with
    /// [`ListError::AuthRequired`](crate::ListError::AuthRequired), and the
    /// list then shows the source's message with a "Sign in" button that
    /// calls `handler`. The picker holds no login code: the application
    /// signs the user in, then calls [`retry`](Self::retry) to list again.
    /// Without a handler the button is not shown.
    pub fn on_auth_required(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.browser.set_auth_handler(Rc::new(handler));
        self
    }

    /// Lists the directory again: the spinner shows and the source is
    /// asked for the first page. Call it after the user signed in, or let
    /// the "Retry" button call it.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.browser.retry(cx);
    }

    /// Why the directory could not be listed, when it could not.
    pub fn failure(&self) -> Option<&crate::ListError> {
        self.browser.failure()
    }

    /// The path field's state.
    pub fn input(&self) -> &Entity<gpui_kit::base::input::InputState> {
        &self.browser.input
    }

    /// The directory that is listed.
    pub fn dir(&self) -> &SourcePath {
        &self.browser.dir
    }

    /// The query: the text after the last separator.
    pub fn query(&self) -> &str {
        &self.browser.query
    }

    /// The path text, as the field shows it.
    pub fn text(&self, cx: &App) -> SharedString {
        self.browser.input.read(cx).value()
    }

    /// What is known about the directory that is listed.
    pub fn listing(&self) -> Option<&Listing> {
        self.browser.listing()
    }

    /// How many folders the list shows, not counting the "Load more"
    /// row.
    pub fn row_count(&self) -> usize {
        self.browser.row_count()
    }

    /// The name of the folder the keyboard is on.
    pub fn highlighted(&self) -> Option<SharedString> {
        self.browser
            .entry_at(self.browser.highlighted?)
            .map(|(entry, _)| entry.name().clone())
    }

    /// The folder "Use folder" would choose: the listed directory while
    /// the query is empty, or the folder the query names exactly. `None`
    /// while the directory is loading or failed, or the query names no
    /// folder.
    pub fn selected(&self) -> Option<SourcePath> {
        let loaded = self.browser.loaded()?;
        let dir = self.browser.dir.clone();
        if self.browser.query.is_empty() {
            Some(dir)
        } else {
            let entry = loaded.entry_named(&self.browser.query)?;
            Some(dir.join(entry.name()))
        }
    }

    /// Chooses [`selected`](Self::selected), if there is one.
    pub fn choose(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.selected() {
            cx.emit(FolderPickerEvent::Chosen(path));
        }
    }

    /// Cancels the picker and every request to the source that is still
    /// running: their answers never arrive. A directory that was still
    /// loading is listed again when the picker shows.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.browser.stop_requests(cx);
        cx.emit(FolderPickerEvent::Cancelled);
    }

    /// Goes to the parent of the listed directory. The root is its own
    /// parent.
    pub fn go_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser.go_up(window, cx);
    }

    /// Goes inside the folder `name` of the listed directory.
    pub fn descend(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.browser.descend(name, window, cx);
    }

    /// Asks the source for the next page of the listed directory, when it
    /// has one and no request is running. After a failure it asks again.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        self.browser.load_more(cx);
    }

    /// Enter: goes inside the highlighted folder, or loads more when the
    /// highlight is on the "Load more" row.
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.browser.highlighted {
            self.enter_row(row, window, cx);
        }
    }

    /// Goes inside the folder at `row`, or loads more for the row after
    /// the last folder.
    fn enter_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        if row == self.browser.row_count() && self.browser.has_more_row() {
            self.browser.load_more(cx);
        } else if let Some(name) = self
            .browser
            .entry_at(row)
            .map(|(entry, _)| entry.name().clone())
        {
            self.browser.descend(&name, window, cx);
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
        let pointer_cursors = Theme::global(cx).pointer_cursors;
        let state = self.state;
        let child = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        if self.open {
            let state = state.clone();
            window.defer(cx, move |_, cx| {
                state.update(cx, |state, cx| state.browser.ensure_listed(cx));
            });
        }
        let (failure, can_sign_in) = {
            let browser = &state.read(cx).browser;
            (browser.failure().cloned(), browser.auth_handler().is_some())
        };
        let (input, list, body_kind, selected, query_empty, has_entries) = {
            let state = state.read(cx);
            let has_entries = state
                .browser
                .loaded()
                .is_some_and(|loaded| !loaded.entries().is_empty());
            (
                state.browser.input.clone(),
                state.browser.list.clone(),
                state.browser.body(),
                state.selected(),
                state.browser.query.is_empty(),
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

        let body = match body_kind {
            Body::Loading => view::loading(child("loading"), &look),
            Body::Failed => match failure {
                Some(error) => view::failure::<FolderEntry, FolderPickerState>(
                    &self.id,
                    &look,
                    &state,
                    &error,
                    can_sign_in,
                ),
                None => div().into_any_element(),
            },
            Body::Empty => view::message(child("message"), &look, view::EMPTY_FOLDERS),
            Body::NoMatch => view::message(child("message"), &look, view::NO_MATCH),
            Body::List => {
                let rows = Rows {
                    id: self.id.clone(),
                    state: state.clone(),
                    look: look.clone(),
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
                .size_full()
                .px(look.row_padding)
                .items_center()
                .text_color(look.muted_foreground)
                .when(pointer_cursors, |this| this.cursor_pointer())
                .child("Select current folder")
                .on_click(move |_, _, cx| state.update(cx, |state, cx| state.choose(cx)))
                .into_any_element()
        });
        let (up_key, down_key, confirm_key) = (state.clone(), state.clone(), state.clone());
        let content = view::stack()
            .key_context(CONTEXT)
            .on_action(move |_: &SelectUp, _, cx| {
                up_key.update(cx, |state, cx| state.browser.move_highlight(-1, cx));
            })
            .on_action(move |_: &SelectDown, _, cx| {
                down_key.update(cx, |state, cx| state.browser.move_highlight(1, cx));
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
            .child(view::slot(&look, current))
            .child(view::list_box(child("list"), &look, list_height, body));

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
    pointer_cursors: bool,
}

impl Rows {
    fn render(&self, row: usize, _: &mut Window, cx: &mut App) -> gpui_kit::AnyElement {
        let look = &self.look;
        let (name, ranges, highlighted) = {
            let state = self.state.read(cx);
            let browser = &state.browser;
            if row == browser.row_count() && browser.has_more_row() {
                return view::more_row::<FolderEntry, FolderPickerState>(
                    &self.id,
                    look,
                    &self.state,
                    browser.more_state(),
                    browser.highlighted == Some(row),
                    row,
                    self.pointer_cursors,
                );
            }
            let Some((entry, ranges)) = browser.entry_at(row) else {
                return div().into_any_element();
            };
            (
                entry.name().clone(),
                ranges.to_vec(),
                browser.highlighted == Some(row),
            )
        };
        let label = crate::menu::matched_text(name.clone(), &ranges, look);
        let state = self.state.clone();
        let hover = self.state.clone();
        view::row_frame(
            ElementId::NamedChild(self.id.clone().into(), name),
            look,
            Role::ListBoxOption,
            highlighted,
            false,
            self.pointer_cursors,
        )
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
                view::highlight_on_hover::<FolderEntry, FolderPickerState>(&hover, row, cx)
            })
        })
        .on_click(move |_, window, cx| {
            state.update(cx, |state, cx| state.enter_row(row, window, cx));
        })
        .into_any_element()
    }
}
