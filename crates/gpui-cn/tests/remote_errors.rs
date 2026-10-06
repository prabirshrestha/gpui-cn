//! A slow or failing remote shows the list's own loading and error states
//! without moving the dialog, never blocks rendering, and is cancelled when
//! the dialog closes. Both pickers, over the fake remote.

mod support;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui_cn::{
    FilePicker, FilePickerEvent, FilePickerState, FolderPicker, FolderPickerEvent,
    FolderPickerState, ListError, ReduceMotion, Theme,
};
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TestAppContext, Window, WindowHandle, base::Root, div, px, size,
    test::TestWindowExt as _,
};
use support::FakeRemote;

enum Picker {
    Folder(Entity<FolderPickerState>),
    File(Entity<FilePickerState>),
}

struct Harness {
    picker: Picker,
    open: Rc<Cell<bool>>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let open = self.open.clone();
        div().size_full().child(match &self.picker {
            Picker::Folder(state) => FolderPicker::new("p", state)
                .open(self.open.get())
                .on_open_change(move |value, _, _| open.set(value))
                .into_any_element(),
            Picker::File(state) => FilePicker::new("p", state)
                .open(self.open.get())
                .on_open_change(move |value, _, _| open.set(value))
                .into_any_element(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Which {
    Folder,
    File,
}

struct Setup {
    handle: WindowHandle<Root>,
    picker: Picker,
    remote: FakeRemote,
    open: Rc<Cell<bool>>,
    folder_events: Rc<RefCell<Vec<FolderPickerEvent>>>,
    file_events: Rc<RefCell<Vec<FilePickerEvent>>>,
    signed_in: Rc<Cell<usize>>,
}

fn start(cx: &mut TestAppContext, which: Which, remote: FakeRemote, with_auth: bool) -> Setup {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_cn::init(cx);
        Theme::update(cx, |theme| theme.reduce_motion = ReduceMotion::On);
    });
    let open = Rc::new(Cell::new(true));
    let folder_events = Rc::new(RefCell::new(Vec::new()));
    let file_events = Rc::new(RefCell::new(Vec::new()));
    let signed_in = Rc::new(Cell::new(0));
    let mut picker = None;
    let source = remote.clone();
    let handle = cx.open_window(size(px(800.), px(800.)), |window, cx| {
        let hook = {
            let remote = remote.clone();
            let signed_in = signed_in.clone();
            move |_: &mut Window, _: &mut gpui_kit::App| {
                signed_in.set(signed_in.get() + 1);
                remote.sign_in();
            }
        };
        let built = match which {
            Which::Folder => {
                let state = cx.new(|cx| {
                    let state = FolderPickerState::new(window, cx).with_source(source, window, cx);
                    let state = state.with_initial("/home/me", window, cx);
                    if with_auth {
                        state.on_auth_required(hook)
                    } else {
                        state
                    }
                });
                let events = folder_events.clone();
                cx.subscribe(&state, move |_, _, event: &FolderPickerEvent, _| {
                    events.borrow_mut().push(event.clone());
                })
                .detach();
                Picker::Folder(state)
            }
            Which::File => {
                let state = cx.new(|cx| {
                    let state = FilePickerState::new(window, cx).with_source(source, window, cx);
                    let state = state.with_initial("/home/me", window, cx);
                    if with_auth {
                        state.on_auth_required(hook)
                    } else {
                        state
                    }
                });
                let events = file_events.clone();
                cx.subscribe(&state, move |_, _, event: &FilePickerEvent, _| {
                    events.borrow_mut().push(event.clone());
                })
                .detach();
                Picker::File(state)
            }
        };
        picker = Some(match &built {
            Picker::Folder(state) => Picker::Folder(state.clone()),
            Picker::File(state) => Picker::File(state.clone()),
        });
        let harness = cx.new(|cx| {
            match &built {
                Picker::Folder(state) => cx.observe(state, |_, _, cx| cx.notify()).detach(),
                Picker::File(state) => cx.observe(state, |_, _, cx| cx.notify()).detach(),
            }
            Harness {
                picker: built,
                open: open.clone(),
            }
        });
        Root::new(harness, window, cx)
    });
    let setup = Setup {
        handle,
        picker: picker.unwrap(),
        remote,
        open,
        folder_events,
        file_events,
        signed_in,
    };
    settle(&setup, cx);
    setup
}

fn settle(setup: &Setup, cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn wait(setup: &Setup, ms: u64, cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    settle(setup, cx);
}

fn part(name: &str) -> ElementId {
    ElementId::NamedChild(
        ElementId::Name("p".into()).into(),
        SharedString::from(name.to_string()),
    )
}

/// The row of an entry: the folder picker names it directly, the file
/// picker under `entry`.
fn row(which: Which, name: &str) -> ElementId {
    match which {
        Which::Folder => part(name),
        Which::File => ElementId::NamedChild(part("entry").into(), name.to_string().into()),
    }
}

fn present(setup: &Setup, id: ElementId, cx: &mut TestAppContext) -> bool {
    cx.update_window(setup.handle.into(), |_, window, _| {
        window.try_find(id).is_some()
    })
    .unwrap()
}

fn click(setup: &Setup, id: ElementId, cx: &mut TestAppContext) {
    cx.update_window(setup.handle.into(), |_, window, cx| window.click(id, cx))
        .unwrap();
    settle(setup, cx);
}

fn list_height(setup: &Setup, cx: &mut TestAppContext) -> gpui_kit::Pixels {
    cx.update_window(setup.handle.into(), |_, window, _| {
        window.find(part("list")).bounds().size.height
    })
    .unwrap()
}

fn dialog_height(setup: &Setup, cx: &mut TestAppContext) -> gpui_kit::Pixels {
    cx.update_window(setup.handle.into(), |_, window, _| {
        window.find(part("list")).bounds().bottom()
    })
    .unwrap()
}

fn retry(setup: &Setup, cx: &mut TestAppContext) {
    cx.update_window(setup.handle.into(), |_, _, cx| match &setup.picker {
        Picker::Folder(state) => state.update(cx, |state, cx| state.retry(cx)),
        Picker::File(state) => state.update(cx, |state, cx| state.retry(cx)),
    })
    .unwrap();
    settle(setup, cx);
}

fn failure(setup: &Setup, cx: &mut TestAppContext) -> Option<ListError> {
    cx.update(|cx| match &setup.picker {
        Picker::Folder(state) => state.read(cx).failure().cloned(),
        Picker::File(state) => state.read(cx).failure().cloned(),
    })
}

fn cancel(setup: &Setup, cx: &mut TestAppContext) {
    setup.open.set(false);
    cx.update_window(setup.handle.into(), |_, _, cx| match &setup.picker {
        Picker::Folder(state) => state.update(cx, |state, cx| state.cancel(cx)),
        Picker::File(state) => state.update(cx, |state, cx| state.cancel(cx)),
    })
    .unwrap();
    settle(setup, cx);
}

const BOTH: [Which; 2] = [Which::Folder, Which::File];

#[gpui_kit::test]
fn a_lost_connection_shows_its_message_and_retry_lists_again(cx: &mut TestAppContext) {
    for which in BOTH {
        let remote = FakeRemote::posix();
        remote.fail_next(ListError::Disconnected(
            "Connection to the server was lost".into(),
        ));
        let setup = start(cx, which, remote, false);
        assert!(present(&setup, part("message"), cx), "{which:?}");
        assert!(present(&setup, part("retry"), cx), "{which:?}");
        assert!(!present(&setup, part("sign-in"), cx));
        assert_eq!(
            failure(&setup, cx),
            Some(ListError::Disconnected(
                "Connection to the server was lost".into()
            ))
        );
        click(&setup, part("retry"), cx);
        assert!(
            !present(&setup, part("message"), cx),
            "{which:?}: recovered"
        );
        assert!(present(&setup, row(which, "code"), cx), "{which:?}: rows");
        assert_eq!(
            setup.remote.requested(),
            ["/home/me", "/home/me"],
            "{which:?}: one request, then one more for the retry"
        );
    }
}

#[gpui_kit::test]
fn a_missing_folder_has_a_message_and_no_action(cx: &mut TestAppContext) {
    let setup = start(cx, Which::Folder, FakeRemote::posix(), false);
    cx.update_window(setup.handle.into(), |_, window, cx| {
        window.press("cmd-a", cx);
        window.press("backspace", cx);
        window.input("/nowhere/", cx);
    })
    .unwrap();
    settle(&setup, cx);
    assert_eq!(failure(&setup, cx), Some(ListError::NotFound));
    assert!(present(&setup, part("message"), cx));
    assert!(!present(&setup, part("retry"), cx));
    assert!(!present(&setup, part("sign-in"), cx));
}

#[gpui_kit::test]
fn a_source_that_needs_a_login_shows_sign_in_and_the_app_decides_the_rest(cx: &mut TestAppContext) {
    for which in BOTH {
        let setup = start(cx, which, FakeRemote::posix().requiring_sign_in(), true);
        assert!(present(&setup, part("sign-in"), cx), "{which:?}");
        assert!(!present(&setup, part("retry"), cx));
        assert!(matches!(
            failure(&setup, cx),
            Some(ListError::AuthRequired(_))
        ));
        click(&setup, part("sign-in"), cx);
        assert_eq!(setup.signed_in.get(), 1, "the app's handler ran");
        assert!(
            present(&setup, part("message"), cx),
            "{which:?}: nothing is listed again until the app says so"
        );
        retry(&setup, cx);
        assert!(!present(&setup, part("message"), cx), "{which:?}");
        assert!(
            present(&setup, row(which, "code"), cx),
            "{which:?}: rows after the login"
        );
        assert_eq!(setup.remote.requested().len(), 2);
    }
}

#[gpui_kit::test]
fn without_a_handler_there_is_no_sign_in_button(cx: &mut TestAppContext) {
    let setup = start(
        cx,
        Which::Folder,
        FakeRemote::posix().requiring_sign_in(),
        false,
    );
    assert!(present(&setup, part("message"), cx));
    assert!(!present(&setup, part("sign-in"), cx));
}

#[gpui_kit::test]
fn a_slow_remote_shows_the_spinner_and_the_dialog_never_moves(cx: &mut TestAppContext) {
    for which in BOTH {
        let remote = FakeRemote::posix().with_latency(Duration::from_millis(400));
        remote.fail_next(ListError::Other("The server said no".into()));
        let setup = start(cx, which, remote, false);
        assert!(
            present(&setup, part("loading"), cx),
            "{which:?}: still loading"
        );
        let height = list_height(&setup, cx);
        let bottom = dialog_height(&setup, cx);
        wait(&setup, 500, cx);
        assert!(present(&setup, part("message"), cx), "{which:?}: failed");
        assert_eq!(list_height(&setup, cx), height, "{which:?}: error");
        assert_eq!(dialog_height(&setup, cx), bottom);
        click(&setup, part("retry"), cx);
        assert!(
            present(&setup, part("loading"), cx),
            "{which:?}: loading again"
        );
        assert_eq!(list_height(&setup, cx), height, "{which:?}: loading again");
        wait(&setup, 500, cx);
        assert!(present(&setup, row(which, "code"), cx), "{which:?}: listed");
        assert_eq!(list_height(&setup, cx), height, "{which:?}: rows");
        assert_eq!(dialog_height(&setup, cx), bottom);
    }
}

#[gpui_kit::test]
fn closing_the_dialog_mid_list_cancels_the_request(cx: &mut TestAppContext) {
    for which in BOTH {
        let remote = FakeRemote::posix().with_latency(Duration::from_millis(400));
        let setup = start(cx, which, remote, false);
        assert_eq!(setup.remote.requested(), ["/home/me"]);
        assert_eq!(setup.remote.cancelled(), 0);
        cancel(&setup, cx);
        assert_eq!(
            setup.remote.cancelled(),
            1,
            "{which:?}: the request was dropped"
        );
        wait(&setup, 1000, cx);
        assert_eq!(
            setup.remote.shared.answered.get(),
            0,
            "{which:?}: nothing answered"
        );
        let events = match which {
            Which::Folder => setup.folder_events.borrow().len(),
            Which::File => setup.file_events.borrow().len(),
        };
        assert_eq!(events, 1, "{which:?}: only the cancel itself is reported");
        setup.open.set(true);
        settle(&setup, cx);
        assert!(present(&setup, part("loading"), cx), "{which:?}: reopened");
        assert_eq!(
            setup.remote.requested(),
            ["/home/me", "/home/me"],
            "listed again"
        );
        wait(&setup, 500, cx);
        assert!(present(&setup, row(which, "code"), cx), "{which:?}: rows");
    }
}

#[gpui_kit::test]
fn leaving_a_slow_folder_cancels_its_request(cx: &mut TestAppContext) {
    let remote = FakeRemote::posix();
    let setup = start(cx, Which::Folder, remote.clone(), false);
    remote.shared.latency.set(Duration::from_millis(400));
    let Picker::Folder(state) = &setup.picker else {
        unreachable!()
    };
    cx.update_window(setup.handle.into(), |_, window, cx| {
        state.update(cx, |state, cx| state.descend("code", window, cx));
    })
    .unwrap();
    settle(&setup, cx);
    assert_eq!(
        remote.requested().last().map(String::as_str),
        Some("/home/me/code")
    );
    cx.update_window(setup.handle.into(), |_, window, cx| {
        state.update(cx, |state, cx| state.go_up(window, cx));
    })
    .unwrap();
    settle(&setup, cx);
    assert_eq!(
        remote.cancelled(),
        1,
        "the request for the folder left was dropped"
    );
    wait(&setup, 500, cx);
    assert!(
        present(&setup, part("code"), cx),
        "back in the parent, listed from cache"
    );
}
