//! A simulated remote machine for the picker stories: a Linux build server
//! held in memory, in the POSIX path style whatever the host is. Its
//! answers take a moment, one folder drops the connection once, one needs
//! a login, and it makes folders. It shows what an application's own
//! source looks like: an SFTP session or an HTTP API would only change
//! where the answers come from.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui_cn::{
    CreateFolderError, FileEntry, FilePage, FileSource, FolderEntry, FolderPage, FolderSource,
    ListError, PageToken, PathStyle, SourcePath,
};
use gpui_kit::{App, Task};

/// The user's home on the server.
pub const HOME: &str = "/home/deploy";
/// A folder whose first listing loses the connection.
pub const FLAKY: &str = "/srv/data";
/// A folder that needs the user to sign in.
pub const LOCKED: &str = "/secure";

/// One entry the server holds: a name, and whether it is a folder.
type Item = (String, bool);

/// One page of entries and where the next page starts.
type Answer = Result<(Vec<Item>, Option<PageToken>), ListError>;

struct Server {
    dirs: HashMap<SourcePath, Vec<Item>>,
    signed_in: bool,
    dropped_once: bool,
    /// Whether answers wait for [`RemoteSim::release`], for a snapshot of
    /// the loading state.
    held: bool,
}

/// The simulated build server. Clones share the machine.
#[derive(Clone)]
pub struct RemoteSim {
    style: PathStyle,
    latency: Duration,
    server: Rc<RefCell<Server>>,
}

fn items(names: &[(&str, bool)]) -> Vec<Item> {
    names
        .iter()
        .map(|(name, folder)| (name.to_string(), *folder))
        .collect()
}

impl RemoteSim {
    /// The server, answering after `latency`.
    pub fn new(latency: Duration) -> Self {
        let style = PathStyle::posix();
        let dir = |path: &str, names: &[(&str, bool)]| (style.path(path), items(names));
        let dirs = HashMap::from([
            dir(
                "/",
                &[
                    ("home", true),
                    ("srv", true),
                    ("secure", true),
                    ("var", true),
                    ("motd", false),
                ],
            ),
            dir("/home", &[("deploy", true)]),
            dir(
                HOME,
                &[
                    ("releases", true),
                    ("shared", true),
                    ("logs", true),
                    (".ssh", true),
                    ("deploy.sh", false),
                    ("notes.txt", false),
                    ("Procfile", false),
                ],
            ),
            dir(
                &format!("{HOME}/releases"),
                &[("2026-10-01", true), ("2026-10-03", true)],
            ),
            dir(&format!("{HOME}/shared"), &[("config.yml", false)]),
            dir(&format!("{HOME}/logs"), &[("app.log", false)]),
            dir("/srv", &[("data", true), ("www", true)]),
            dir(
                FLAKY,
                &[("backups", true), ("exports", true), ("report.csv", false)],
            ),
            dir(LOCKED, &[("keys", true), ("audit.log", false)]),
        ]);
        Self {
            style,
            latency,
            server: Rc::new(RefCell::new(Server {
                dirs,
                signed_in: false,
                dropped_once: false,
                held: false,
            })),
        }
    }

    /// Puts the server back as it started: signed out, with the flaky
    /// folder due to drop its connection again, and answering.
    pub fn reset(&self) {
        let mut machine = self.server.borrow_mut();
        machine.signed_in = false;
        machine.dropped_once = false;
        machine.held = false;
    }

    /// Makes answers wait until [`release`](Self::release).
    pub fn hold(&self) {
        self.server.borrow_mut().held = true;
    }

    /// Lets the answers go, and every later one answers after the latency.
    pub fn release(&self) {
        self.server.borrow_mut().held = false;
    }

    /// Signs the user in, which opens the locked folder.
    pub fn sign_in(&self) {
        self.server.borrow_mut().signed_in = true;
    }

    /// One request: it waits, then answers with a dropped connection the
    /// first time the flaky folder is asked for, a login demand for the
    /// locked folder until the user signed in, or the folder's entries.
    fn request(&self, dir: &SourcePath, page: Option<PageToken>, cx: &mut App) -> Task<Answer> {
        let (server, latency, dir) = (self.server.clone(), self.latency, dir.clone());
        let style = self.style;
        cx.spawn(async move |cx| {
            if server.borrow().held {
                std::future::pending::<()>().await;
            }
            cx.background_executor().timer(latency).await;
            let mut machine = server.borrow_mut();
            if dir == style.path(FLAKY) && !machine.dropped_once {
                machine.dropped_once = true;
                return Err(ListError::Disconnected(
                    "The connection to build-server was lost".into(),
                ));
            }
            if dir == style.path(LOCKED) && !machine.signed_in {
                return Err(ListError::AuthRequired(
                    "Sign in to build-server to open this folder".into(),
                ));
            }
            let found = machine.dirs.get(&dir).cloned().ok_or(ListError::NotFound)?;
            let start: usize = page
                .and_then(|token| token.as_str().parse().ok())
                .unwrap_or(0);
            let end = (start + 6).min(found.len());
            let next = (end < found.len()).then(|| PageToken::new(end.to_string()));
            Ok((found[start..end].to_vec(), next))
        })
    }

    fn make(&self, parent: &SourcePath, name: &str) -> Result<SourcePath, CreateFolderError> {
        let mut machine = self.server.borrow_mut();
        let style = self.style;
        let Some(found) = machine.dirs.get_mut(parent) else {
            return Err(CreateFolderError::PermissionDenied);
        };
        if found
            .iter()
            .any(|(known, _)| style.fold(known) == style.fold(name))
        {
            return Err(CreateFolderError::Exists);
        }
        found.push((name.to_string(), true));
        found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| style.compare_names(&a.0, &b.0)));
        let path = parent.join(name);
        machine.dirs.insert(path.clone(), Vec::new());
        Ok(path)
    }
}

impl FolderSource for RemoteSim {
    fn path_style(&self) -> PathStyle {
        self.style
    }

    fn home(&self) -> Option<SourcePath> {
        Some(self.style.path(HOME))
    }

    fn list(
        &self,
        dir: &SourcePath,
        page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<Result<FolderPage, ListError>> {
        let task = self.request(dir, page, cx);
        let style = self.style;
        cx.spawn(async move |_| {
            let (found, next) = task.await?;
            let page = FolderPage::new(found.into_iter().filter(|(_, folder)| *folder).map(
                |(name, _)| {
                    let hidden = style.is_hidden_name(&name);
                    FolderEntry::new(name).with_hidden(hidden)
                },
            ));
            Ok(match next {
                Some(token) => page.with_next(token),
                None => page,
            })
        })
    }

    fn can_create_folders(&self) -> bool {
        true
    }

    fn create_folder(
        &self,
        parent: &SourcePath,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<SourcePath, CreateFolderError>> {
        let result = self.make(parent, name);
        let latency = self.latency;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(latency).await;
            result
        })
    }
}

impl FileSource for RemoteSim {
    fn path_style(&self) -> PathStyle {
        self.style
    }

    fn home(&self) -> Option<SourcePath> {
        Some(self.style.path(HOME))
    }

    fn list(
        &self,
        dir: &SourcePath,
        page: Option<PageToken>,
        cx: &mut App,
    ) -> Task<Result<FilePage, ListError>> {
        let task = self.request(dir, page, cx);
        let style = self.style;
        cx.spawn(async move |_| {
            let (found, next) = task.await?;
            let page = FilePage::new(found.into_iter().map(|(name, folder)| {
                let hidden = style.is_hidden_name(&name);
                if folder {
                    FileEntry::folder(name).with_hidden(hidden)
                } else {
                    FileEntry::file(name).with_hidden(hidden).with_size(2_048)
                }
            }));
            Ok(match next {
                Some(token) => page.with_next(token),
                None => page,
            })
        })
    }

    fn can_create_folders(&self) -> bool {
        true
    }

    fn create_folder(
        &self,
        parent: &SourcePath,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<SourcePath, CreateFolderError>> {
        let result = self.make(parent, name);
        let latency = self.latency;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(latency).await;
            result
        })
    }
}
