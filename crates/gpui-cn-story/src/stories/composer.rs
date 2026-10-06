use std::sync::Arc;

use gpui_cn::{
    ActiveTheme as _, Attachment, AttachmentStrip, Button, ButtonSize, Composer, ComposerEvent,
    ComposerState, ComposerStatusTab, ModelEntry, ModelPicker, ModelPickerState, ModelProvider,
    PermissionMenu, PermissionMode, PermissionState, gpui_kit::assets::IconName,
};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, Image, ImageFormat, ImageSource, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};

use crate::{Story, agents, note, page, section};

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
    permission: Entity<PermissionState>,
    custom_permission: Entity<PermissionState>,
    models: Entity<ModelPickerState>,
    custom_models: Entity<ModelPickerState>,
    basic: Entity<ComposerState>,
    rich: Entity<ComposerState>,
    custom: Entity<ComposerState>,
    log: SharedString,
    agent: Entity<ComposerState>,
}

impl ComposerStory {
    /// A made-up catalog: the picker ships no vendor marks, so each
    /// provider brings any icon.
    fn catalog() -> Vec<ModelProvider> {
        vec![
            ModelProvider::new("orbit", "Orbit")
                .icon(IconName::Bot)
                .models([
                    ModelEntry::new("orbit-mini", "Orbit Mini"),
                    ModelEntry::new("orbit-pro", "Orbit Pro").effort(true),
                    ModelEntry::new("orbit-max", "Orbit Max").effort(true),
                    ModelEntry::new("orbit-lite", "Orbit Lite"),
                    ModelEntry::new("orbit-edge", "Orbit Edge"),
                    ModelEntry::new("orbit-old", "Orbit 1"),
                    ModelEntry::new("orbit-older", "Orbit 0"),
                ]),
            ModelProvider::new("lumen", "Lumen")
                .icon(IconName::Sun)
                .models([
                    ModelEntry::new("lumen-1", "Lumen One"),
                    ModelEntry::new("lumen-2", "Lumen Two").effort(true),
                ]),
            ModelProvider::new("nova", "Nova")
                .icon(IconName::Star)
                .models([
                    ModelEntry::new("nova-lite", "Nova Lite"),
                    ModelEntry::new("nova-edge", "Nova Edge"),
                ]),
            ModelProvider::new("tide", "Tide")
                .icon(IconName::Globe)
                .models([ModelEntry::new("tide-s", "Tide S")]),
            ModelProvider::new("ember", "Ember")
                .icon(IconName::Heart)
                .models([ModelEntry::new("ember-1", "Ember One")]),
            ModelProvider::new("dusk", "Dusk")
                .icon(IconName::Moon)
                .models([ModelEntry::new("dusk-1", "Dusk One")]),
            ModelProvider::new("grid", "Grid")
                .icon(IconName::Network)
                .models([ModelEntry::new("grid-1", "Grid One")]),
            ModelProvider::new("chip", "Chip")
                .icon(IconName::Cpu)
                .models([ModelEntry::new("chip-1", "Chip One")]),
            ModelProvider::new("paint", "Paint")
                .icon(IconName::Palette)
                .models([ModelEntry::new("paint-1", "Paint One")]),
        ]
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
            ComposerEvent::ModelChanged(id) => format!("Model: {id}"),
            ComposerEvent::EffortChanged(level) => format!("Effort level: {level}"),
            ComposerEvent::AddClicked => "Add pressed".to_string(),
            ComposerEvent::MicClicked => "Mic pressed".to_string(),
            ComposerEvent::FilesDropped(paths) => format!("Dropped {} file(s)", paths.len()),
            _ => "Event".to_string(),
        }
        .into()
    }

    fn sample() -> Vec<Attachment> {
        vec![
            Attachment::new("photo", "photo.png").image(checker()),
            Attachment::new("notes", "meeting-notes.md"),
            Attachment::new("sheet", "forecast-q3.xlsx").progress(55.),
            Attachment::new("deck", "launch.pptx").progress(0.),
            Attachment::new("code", "main.rs"),
            Attachment::new("clip", "demo.mov"),
        ]
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
            let custom_permission = cx.new(|cx| {
                PermissionState::new(cx).with_modes([
                    PermissionMode::new("read", "Read only")
                        .description("Look at files, never change them")
                        .icon(IconName::Eye),
                    PermissionMode::new("edit", "Edit files")
                        .description("Change files in this folder")
                        .icon(IconName::File),
                    PermissionMode::new("shell", "Run commands")
                        .description("Run commands in the terminal")
                        .icon(IconName::SquareTerminal),
                ])
            });
            let models = cx.new(|cx| {
                ModelPickerState::new(Self::catalog(), window, cx).with_selected("orbit-pro")
            });
            let custom_models = cx.new(|cx| {
                ModelPickerState::new(
                    [ModelProvider::new("local", "On this machine")
                        .icon(IconName::HardDrive)
                        .models([
                            ModelEntry::new("tiny", "Tiny 1B"),
                            ModelEntry::new("small", "Small 8B").effort(true),
                        ])],
                    window,
                    cx,
                )
            });
            let basic = cx.new(|cx| ComposerState::new(window, cx));
            let rich_models = cx.new(|cx| {
                ModelPickerState::new(Self::catalog(), window, cx).with_selected("orbit-pro")
            });
            let rich =
                cx.new(|cx| ComposerState::new(window, cx).with_models(rich_models, window, cx));
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
                ComposerState::new(window, cx)
                    .with_permission(custom_permission.clone(), window, cx)
                    .with_models(custom_models.clone(), window, cx)
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
            cx.observe(&agent, |_: &mut Self, _, cx| cx.notify())
                .detach();
            cx.observe(&models, |_, _, cx| cx.notify()).detach();
            cx.observe(&custom_models, |_, _, cx| cx.notify()).detach();
            cx.observe(&permission, |_, _, cx| cx.notify()).detach();
            cx.observe(&custom_permission, |_, _, cx| cx.notify())
                .detach();
            Self {
                attachments: Self::sample(),
                permission,
                custom_permission,
                models,
                custom_models,
                basic,
                rich,
                custom,
                log: "Nothing yet. Type a prompt and press Enter.".into(),
                agent,
            }
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
        let chosen = |state: &Entity<ModelPickerState>| {
            state.read(cx).selected_model().map_or_else(
                || "none".to_string(),
                |(_, model)| match state.read(cx).effort_label() {
                    Some(effort) => format!("{} ({effort})", model.name()),
                    None => model.name().to_string(),
                },
            )
        };
        let (model, custom_model) = (chosen(&self.models), chosen(&self.custom_models));
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
                        "The model picker's rail switches between the three providers of \
                         the example catalog, and the permission menu holds a Claude Code \
                         style set. Both are plain data in the story, and the application's \
                         to change.",
                        cx,
                    ))
                    .child(
                        Composer::new("composer-agent", &self.agent).status(
                            ComposerStatusTab::new("composer-agent-status")
                                .branch("feature/composer")
                                .folder("gpui-cn")
                                .context(57.),
                        ),
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
                        Composer::new("composer-rich", &self.rich).status(
                            ComposerStatusTab::new("composer-rich-status")
                                .branch("Main")
                                .folder("project-sea")
                                .context(57.),
                        ),
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
                        "Every control is the application's to replace or leave out: here the \
                         microphone is gone, a web button joins the toolbar, and the model \
                         catalog and the permission modes are the application's own.",
                        cx,
                    ))
                    .child(
                        Composer::new("composer-custom", &self.custom)
                            .placeholder("Ask the local model")
                            .max_lines(4)
                            .show_mic(false)
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
                         picture. An uploading tile draws an arc around its border and counts \
                         the percent; it swaps the counter for the dismiss button when the \
                         upload ends.",
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
                                Button::new("finish")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Finish uploads")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        for a in &mut this.attachments {
                                            a.set_progress(None);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("reset")
                                    .outline()
                                    .size(ButtonSize::Sm)
                                    .label("Reset")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.attachments = Self::sample();
                                        cx.notify();
                                    })),
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
                        "The strip that sits behind the top of a composer card: a branch and a \
                         folder on the left, the context meter at the right. The meter's percent \
                         is a plain value the application sets.",
                        cx,
                    ))
                    .child(
                        ComposerStatusTab::new("status-default")
                            .branch("Main")
                            .folder("project-sea")
                            .context(57.),
                    )
                    .child(
                        ComposerStatusTab::new("status-custom")
                            .branch("feature/composer")
                            .context(92.),
                    )
                    .child(note(
                        "Over a card stub the tab keeps its own 34px and the card's hairline \
                         covers its bottom edge.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                ComposerStatusTab::new("status-joined")
                                    .branch("Main")
                                    .folder("project-sea")
                                    .context(57.),
                            )
                            .child(
                                div()
                                    .h(px(64.))
                                    .mt(-px(1.))
                                    .rounded(cx.theme().metrics.composer_radius)
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
            section(
                "Model picker",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "A rail of providers, a quick search across all of them (press / to \
                         focus it), and the models. The chosen model shows an effort chip. The \
                         catalog and the marks are the application's, and any trigger opens it.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(ModelPicker::new("models", &self.models))
                            .child(note(format!("Model: {model}"), cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                ModelPicker::new("custom-models", &self.custom_models).trigger(
                                    Button::new("custom-models-trigger")
                                        .outline()
                                        .size(ButtonSize::Sm)
                                        .icon(IconName::Cpu)
                                        .label("Choose a local model"),
                                ),
                            )
                            .child(note(format!("Model: {custom_model}"), cx)),
                    ),
            )
            .into_any_element(),
        ])
    }
}
