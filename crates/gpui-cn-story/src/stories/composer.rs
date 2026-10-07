use std::{sync::Arc, time::Duration};

use gpui_cn::{
    ActiveTheme as _, Attachment, AttachmentStrip, Button, ButtonSize, Composer, ComposerEvent,
    ComposerState, ComposerStatusTab, ModelEntry, ModelPickerState, ModelProvider, PermissionMenu,
    PermissionMode, PermissionState, StatusSelect, gpui_kit::assets::IconName,
};
use gpui_kit::base::Disableable as _;
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, Image, ImageFormat, ImageSource, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Task, Window, div, px,
};

use crate::{Story, agents, note, page, section};

/// How often the simulated upload moves, and by how much: 5% every 100ms
/// is two seconds from nothing to done.
const UPLOAD_TICK: Duration = Duration::from_millis(100);
const UPLOAD_STEP: f32 = 5.;
/// The pause between one file landing and the next starting.
const UPLOAD_GAP: Duration = Duration::from_millis(300);

/// A checkerboard drawn as SVG, so the story needs no picture file.
const CHECKER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4" fill="#5b8def"/><rect width="1" height="1" fill="#8fb1f7"/><rect x="2" width="1" height="1" fill="#8fb1f7"/><rect y="1" x="1" width="1" height="1" fill="#8fb1f7"/><rect y="1" x="3" width="1" height="1" fill="#8fb1f7"/><rect y="2" width="1" height="1" fill="#8fb1f7"/><rect y="2" x="2" width="1" height="1" fill="#8fb1f7"/><rect y="3" x="1" width="1" height="1" fill="#8fb1f7"/><rect y="3" x="3" width="1" height="1" fill="#8fb1f7"/></svg>"##;

fn checker() -> ImageSource {
    ImageSource::from(Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        CHECKER.as_bytes().to_vec(),
    )))
}

/// The composer family: the attachment strip, the pickers, and the
/// composer that holds them.
pub struct ComposerStory {
    attachments: Vec<Attachment>,
    /// The running simulated upload. Dropping it stops the ticks.
    upload: Option<Task<()>>,
    permission: Entity<PermissionState>,
    custom_permission: Entity<PermissionState>,
    basic: Entity<ComposerState>,
    rich: Entity<ComposerState>,
    custom: Entity<ComposerState>,
    log: SharedString,
    agent: Entity<ComposerState>,
    agent_status: agents::SampleStatus,
    rich_status: agents::SampleStatus,
    tab_status: agents::SampleStatus,
    joined_status: agents::SampleStatus,
    narrow_status: agents::SampleStatus,
    short_status: agents::SampleStatus,
    wide_status: agents::SampleStatus,
    narrow_composer_status: agents::SampleStatus,
    narrow_composer: Entity<ComposerState>,
    tiny_status: agents::SampleStatus,
    tiny_composer_status: agents::SampleStatus,
    tiny_composer: Entity<ComposerState>,
    branch_only: agents::SampleStatus,
}

impl ComposerStory {
    /// A tiny catalog for the examples that show a custom one: the picker
    /// ships no vendor marks, so a provider brings any icon.
    fn custom_catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("local", "On this machine")
                .icon(IconName::HardDrive)
                .models([
                    ModelEntry::new("tiny", "Tiny 1B"),
                    ModelEntry::new("small", "Small 8B").effort(true),
                ]),
        ]
    }

    fn custom_models<T: 'static>(
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Entity<ModelPickerState> {
        cx.new(|cx| {
            ModelPickerState::new(Self::custom_catalog(), window, cx).with_selected("small")
        })
    }

    fn custom_modes() -> Vec<PermissionMode> {
        vec![
            PermissionMode::new("read", "Read only")
                .description("Look at files, never change them")
                .icon(IconName::Eye),
            PermissionMode::new("edit", "Edit files")
                .description("Change files in this folder")
                .icon(IconName::File),
            PermissionMode::new("shell", "Run commands")
                .description("Run commands in the terminal")
                .icon(IconName::SquareTerminal),
        ]
    }

    /// Every composer example with the picker it owns, and whether its
    /// catalog is the labelled custom one.
    pub fn composers(&self, cx: &App) -> Vec<(&'static str, Entity<ModelPickerState>, bool)> {
        [
            ("agent", &self.agent, false),
            ("basic", &self.basic, false),
            ("rich", &self.rich, false),
            ("custom", &self.custom, true),
        ]
        .into_iter()
        .map(|(name, composer, custom)| (name, composer.read(cx).models().clone(), custom))
        .collect()
    }

    fn describe(event: &ComposerEvent) -> SharedString {
        match event {
            ComposerEvent::Submit { text, attachments } => {
                format!(
                    "Submit: \"{text}\" with {} attachment(s)",
                    attachments.len()
                )
            }
            ComposerEvent::AttachmentsChanged => "Attachments changed".to_string(),
            ComposerEvent::PermissionChanged(id) => format!("Permission mode: {id}"),
            ComposerEvent::PermissionLearnMore => "Permission: learn more".to_string(),
            ComposerEvent::StatusChanged { item, value } => format!("{item}: {value}"),
            ComposerEvent::ModelChanged(id) => format!("Model: {id}"),
            ComposerEvent::EffortChanged(level) => format!("Effort level: {level}"),
            ComposerEvent::AddClicked => "Add pressed".to_string(),
            ComposerEvent::MicClicked => "Mic pressed".to_string(),
            ComposerEvent::FilesDropped(paths) => format!("Dropped {} file(s)", paths.len()),
            _ => "Event".to_string(),
        }
        .into()
    }

    /// The list the upload demo starts from: a landed image, one file
    /// uploading, two queued at zero, and two more that are ready.
    pub fn sample() -> Vec<Attachment> {
        vec![
            Attachment::new("photo", "photo.png").image(checker()),
            Attachment::new("notes", "meeting-notes.md"),
            Attachment::new("sheet", "forecast-q3.xlsx").progress(55.),
            Attachment::new("deck", "launch.pptx").progress(0.),
            Attachment::new("report", "report.pdf").progress(0.),
            Attachment::new("code", "main.rs"),
        ]
    }

    /// The attachments of the upload demo.
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// Whether the simulated upload is running.
    pub fn is_uploading(&self) -> bool {
        self.upload.is_some()
    }

    /// Runs the upload queue: one file at a time, from where it is to
    /// 100 over about two seconds, with a short gap between files. Each
    /// file's progress is cleared when it lands. The task ticks only while
    /// a file uploads and ends with the queue.
    pub fn start_upload(&mut self, cx: &mut Context<Self>) {
        self.upload = Some(cx.spawn(async move |this, cx| {
            loop {
                let next = this
                    .update(cx, |this, _| {
                        this.attachments
                            .iter()
                            .find_map(|a| a.progress_percent().map(|p| (a.id().clone(), p)))
                    })
                    .ok()
                    .flatten();
                let Some((id, mut progress)) = next else {
                    break;
                };
                while progress < 100. {
                    cx.background_executor().timer(UPLOAD_TICK).await;
                    progress = (progress + UPLOAD_STEP).min(100.);
                    let alive = this.update(cx, |this, cx| {
                        if let Some(a) = this.attachments.iter_mut().find(|a| a.id() == &id) {
                            a.set_progress(Some(progress));
                        }
                        cx.notify();
                    });
                    if alive.is_err() {
                        return;
                    }
                }
                this.update(cx, |this, cx| {
                    if let Some(a) = this.attachments.iter_mut().find(|a| a.id() == &id) {
                        a.set_progress(None);
                    }
                    cx.notify();
                })
                .ok();
                cx.background_executor().timer(UPLOAD_GAP).await;
            }
            this.update(cx, |this, cx| {
                this.upload = None;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Stops the upload and restores the list it started from.
    pub fn reset_upload(&mut self, cx: &mut Context<Self>) {
        self.upload = None;
        self.attachments = Self::sample();
        cx.notify();
    }
}

impl Story for ComposerStory {
    fn title() -> &'static str {
        "Composer"
    }

    fn icon() -> IconName {
        IconName::Bot
    }

    fn description() -> &'static str {
        "A prompt box with attachments, a permission mode, and a model picker."
    }

    fn view(window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|cx| {
            let permission = cx.new(PermissionState::new);
            let custom_permission =
                cx.new(|cx| PermissionState::new(cx).with_modes(Self::custom_modes()));
            let basic = cx.new(|cx| {
                let models = agents::model_picker(window, cx);
                ComposerState::new(window, cx).with_models(models, window, cx)
            });
            let rich = cx.new(|cx| {
                let models = agents::model_picker(window, cx);
                ComposerState::new(window, cx).with_models(models, window, cx)
            });
            rich.update(cx, |state, cx| {
                for attachment in [
                    Attachment::new("photo", "photo.png").image(checker()),
                    Attachment::new("notes", "meeting-notes.md"),
                    Attachment::new("sheet", "forecast-q3.xlsx").progress(55.),
                ] {
                    state.add_attachment(attachment, cx);
                }
            });
            let custom = cx.new(|cx| {
                let models = Self::custom_models(window, cx);
                let permission =
                    cx.new(|cx| PermissionState::new(cx).with_modes(Self::custom_modes()));
                ComposerState::new(window, cx)
                    .with_permission(permission, window, cx)
                    .with_models(models, window, cx)
            });
            for composer in [&basic, &rich, &custom] {
                cx.subscribe(composer, |this: &mut Self, _, event: &ComposerEvent, cx| {
                    this.log = Self::describe(event);
                    cx.notify();
                })
                .detach();
                cx.observe(composer, |_: &mut Self, _, cx| cx.notify())
                    .detach();
            }
            let agent = agents::composer(window, cx);
            let agent_status = agents::SampleStatus::new(cx);
            let rich_status = agents::SampleStatus::new(cx);
            let tab_status = agents::SampleStatus::new(cx);
            let joined_status = agents::SampleStatus::new(cx);
            let branch_only = agents::SampleStatus::new(cx);
            let narrow_status = agents::SampleStatus::long(cx);
            let short_status = agents::SampleStatus::short(cx);
            let wide_status = agents::SampleStatus::long(cx);
            let narrow_composer_status = agents::SampleStatus::long(cx);
            let narrow_composer = agents::composer(window, cx);
            narrow_composer_status.watch(&narrow_composer, cx);
            let tiny_status = agents::SampleStatus::long(cx);
            let tiny_composer_status = agents::SampleStatus::long(cx);
            let tiny_composer = agents::composer(window, cx);
            tiny_composer_status.watch(&tiny_composer, cx);
            cx.observe(&tiny_composer, |_, _, cx| cx.notify()).detach();
            cx.observe(&narrow_composer, |_, _, cx| cx.notify())
                .detach();
            agent_status.watch(&agent, cx);
            rich_status.watch(&rich, cx);
            for status in [
                &agent_status,
                &rich_status,
                &tab_status,
                &joined_status,
                &branch_only,
                &narrow_status,
                &short_status,
                &wide_status,
                &narrow_composer_status,
                &tiny_status,
                &tiny_composer_status,
            ] {
                for state in [&status.project, &status.device, &status.branch] {
                    cx.observe(state, |_: &mut Self, _, cx| cx.notify())
                        .detach();
                }
            }
            cx.observe(&agent, |_: &mut Self, _, cx| cx.notify())
                .detach();
            cx.observe(&permission, |_, _, cx| cx.notify()).detach();
            cx.observe(&custom_permission, |_, _, cx| cx.notify())
                .detach();
            let mut story = Self {
                attachments: Self::sample(),
                upload: None,
                permission,
                custom_permission,
                basic,
                rich,
                custom,
                log: "Nothing yet. Type a prompt and press Enter.".into(),
                agent,
                agent_status,
                rich_status,
                tab_status,
                joined_status,
                branch_only,
                narrow_status,
                short_status,
                wide_status,
                narrow_composer_status,
                narrow_composer,
                tiny_status,
                tiny_composer_status,
                tiny_composer,
            };
            story.start_upload(cx);
            story
        })
        .into()
    }
}

impl Render for ComposerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chosen = |state: &Entity<PermissionState>| {
            state
                .read(cx)
                .selected_mode()
                .map_or_else(String::new, |mode| mode.label().to_string())
        };
        let (mode, custom_mode) = (chosen(&self.permission), chosen(&self.custom_permission));
        let log = self.log.clone();
        page([
            section(
                "Coding agents",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(560.))
                    .child(note(
                        "The model picker's rail switches between the two providers of \
                         the example catalog, and the permission menu holds a Claude Code \
                         style set. Both are plain data in the story, and the application's \
                         to change.",
                        cx,
                    ))
                    .child(
                        Composer::new("composer-agent", &self.agent)
                            .placeholder("Do anything")
                            .status(self.agent_status.tab("composer-agent-status")),
                    ),
            )
            .into_any_element(),
            section(
                "Composer",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(560.))
                    .child(note(
                        "Enter sends and Shift+Enter starts a new line. The prompt grows to ten \
                         lines. The card is bare: no status tab.",
                        cx,
                    ))
                    .child(Composer::new("composer-basic", &self.basic))
                    .child(note(format!("Last event: {log}"), cx)),
            )
            .into_any_element(),
            section(
                "With status and attachments",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(560.))
                    .child(note(
                        "The status tab sits behind the card. An uploading file blocks the send \
                         button until its progress clears. Drop files on the card to add more.",
                        cx,
                    ))
                    .child(
                        Composer::new("composer-rich", &self.rich)
                            .status(self.rich_status.tab("composer-rich-status").context(57.)),
                    )
                    .child(
                        div().flex().gap_2().child(
                            Button::new("rich-finish")
                                .outline()
                                .size(ButtonSize::Sm)
                                .label("Finish upload")
                                .on_click({
                                    let rich = self.rich.clone();
                                    move |_, _, cx| {
                                        rich.update(cx, |state, cx| {
                                            state.set_attachment_progress("sheet", None, cx)
                                        })
                                    }
                                }),
                        ),
                    ),
            )
            .into_any_element(),
            section(
                "Custom catalog, modes, and controls",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(560.))
                    .child(note(
                        "Every control is the application's to replace or leave out: here a web \
                         button joins the toolbar, and the model \
                         catalog and the permission modes are the application's own.",
                        cx,
                    ))
                    .child(
                        Composer::new("composer-custom", &self.custom)
                            .placeholder("Ask the local model")
                            .max_lines(4)
                            .toolbar(
                                Button::new("web")
                                    .ghost()
                                    .size(ButtonSize::Sm)
                                    .icon(IconName::Globe)
                                    .label("Web")
                                    .tooltip("Search the web"),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Attachments",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(340.))
                    .child(note(
                        "A file tile shows a tinted icon and its name. An image shows its \
                         picture. A queued or uploading tile draws a ring at its top left; an \
                         uploading one also counts the percent at its top right, and swaps the \
                         counter for the dismiss button when the upload ends.",
                        cx,
                    ))
                    .child(
                        AttachmentStrip::new("files")
                            .attachments(self.attachments.clone())
                            .on_dismiss(cx.listener(|this, id, _, cx| {
                                this.attachments.retain(|a| a.id() != id);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("start-upload")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Start upload")
                                    .disabled(self.upload.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| this.start_upload(cx))),
                            )
                            .child(
                                Button::new("reset")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Reset")
                                    .on_click(cx.listener(|this, _, _, cx| this.reset_upload(cx))),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Menu or search",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(560.))
                    .child(note(
                        "A dropdown with three options or more opens a list with a search \
                         field that ranks what is typed, and a shorter one opens a plain \
                         menu. Here the project and the branch have three options, so they \
                         search, and the device has two, so it is a menu. The tabs above have \
                         twelve projects and thirty refs.",
                        cx,
                    ))
                    .child(self.short_status.tab("status-short").context(57.)),
            )
            .into_any_element(),
            section(
                "Narrow widths",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Items keep one gap and stay left. When the row is too narrow the labels \
                         end in an ellipsis, the chevron stays beside its label, and the meter \
                         keeps its place at the right. The toolbar below behaves the same.",
                        cx,
                    ))
                    .child(
                        div()
                            .w(px(200.))
                            .child(self.tiny_status.tab("status-tiny").context(57.)),
                    )
                    .child(
                        div()
                            .w(px(320.))
                            .child(self.narrow_status.tab("status-narrow").context(57.)),
                    )
                    .child(
                        div()
                            .w(px(480.))
                            .child(self.wide_status.tab("status-medium").context(57.)),
                    )
                    .child(
                        div().w(px(240.)).child(
                            Composer::new("composer-tiny", &self.tiny_composer).status(
                                self.tiny_composer_status
                                    .tab("composer-tiny-status")
                                    .context(57.),
                            ),
                        ),
                    )
                    .child(
                        div().w(px(340.)).child(
                            Composer::new("composer-narrow", &self.narrow_composer).status(
                                self.narrow_composer_status
                                    .tab("composer-narrow-status")
                                    .context(57.),
                            ),
                        ),
                    ),
            )
            .into_any_element(),
            section(
                "Status tab",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(520.))
                    .child(note(
                        "The strip that sits behind the top of a composer card: dropdowns for a \
                         project, a device, and a branch on the left, and optionally the context \
                         meter at the right. The meter's percent is a plain value the \
                         application sets.",
                        cx,
                    ))
                    .child(self.tab_status.tab("status-default").context(57.))
                    .child(
                        ComposerStatusTab::new("status-custom")
                            .select(
                                StatusSelect::new("status-custom-branch", &self.branch_only.branch)
                                    .icon(IconName::GitBranch),
                            )
                            .context(92.),
                    )
                    .child(note(
                        "Over a card stub the tab keeps its own 38px and the card's hairline \
                         covers its bottom edge.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.joined_status.tab("status-joined").context(57.))
                            .child(
                                div()
                                    .h(px(64.))
                                    .mt(-px(1.))
                                    .rounded(cx.theme().radius_xl())
                                    .border_1()
                                    .border_color(cx.theme().border())
                                    .bg(cx.theme().base.colors.surface),
                            ),
                    ),
            )
            .into_any_element(),
            section(
                "Permission menu",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "A quiet trigger over a menu of modes with a line of description each. \
                         The modes are the application's: the second menu defines its own.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(PermissionMenu::new("permission", &self.permission))
                            .child(note(format!("Mode: {mode}"), cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                PermissionMenu::new("custom-permission", &self.custom_permission)
                                    .learn_more(false),
                            )
                            .child(note(format!("Mode: {custom_mode}"), cx)),
                    ),
            )
            .into_any_element(),
        ])
    }
}
