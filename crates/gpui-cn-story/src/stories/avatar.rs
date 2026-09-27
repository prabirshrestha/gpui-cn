use gpui_cn::{Avatar, AvatarGroup, AvatarSize, gpui_kit::assets::IconName};
use std::sync::Arc;

use gpui_kit::{
    AnyView, App, AppContext as _, Context, Image, ImageFormat, ImageSource, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div,
};

use crate::{Story, note, page, section};

/// Avatars with images, initials, and icons, alone and in groups.
pub struct AvatarStory;

impl Story for AvatarStory {
    fn title() -> &'static str {
        "Avatar"
    }

    fn icon() -> IconName {
        IconName::CircleUser
    }

    fn description() -> &'static str {
        "An image element with a fallback for representing the user."
    }

    fn view(_: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| Self).into()
    }
}

/// The people the story shows, by id and name.
const PEOPLE: [(&str, &str); 5] = [
    ("ada", "Ada Lovelace"),
    ("alan", "Alan Turing"),
    ("grace", "Grace Hopper"),
    ("linus", "Linus Torvalds"),
    ("margaret", "Margaret Hamilton"),
];

fn people() -> impl Iterator<Item = Avatar> {
    PEOPLE
        .into_iter()
        .map(|(id, name)| Avatar::new(format!("group-{id}")).name(name))
}

/// A portrait drawn as an SVG, so the story shows real images without a
/// network or a bundled photo: a head and shoulders on a gradient.
fn portrait(from: &str, to: &str) -> ImageSource {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
<stop offset="0" stop-color="{from}"/><stop offset="1" stop-color="{to}"/>
</linearGradient></defs>
<rect width="64" height="64" fill="url(#g)"/>
<circle cx="32" cy="25" r="11" fill="#fff" fill-opacity="0.85"/>
<path d="M10 64c0-13 10-22 22-22s22 9 22 22z" fill="#fff" fill-opacity="0.85"/>
</svg>"##
    );
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())).into()
}

/// The three sizes, smallest first, with a suffix for their ids.
const SIZES: [(AvatarSize, &str); 3] = [
    (AvatarSize::Sm, "sm"),
    (AvatarSize::Default, "md"),
    (AvatarSize::Lg, "lg"),
];

impl Render for AvatarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = || div().flex().items_center().gap_4();
        let sizes =
            |kind: &'static str, avatar: fn(Avatar) -> Avatar| {
                row().children(SIZES.map(|(size, suffix)| {
                    avatar(Avatar::new(format!("{kind}-{suffix}")).size(size))
                }))
            };
        page([
            section(
                "Image",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "The initials show while the image loads and stay if it fails.",
                        cx,
                    ))
                    .child(sizes("image", |avatar| {
                        avatar
                            .src(portrait("#6366f1", "#ec4899"))
                            .name("Ada Lovelace")
                    }))
                    .child(sizes("broken", |avatar| {
                        avatar.src("missing/avatar.png").name("Ada Lovelace")
                    })),
            )
            .into_any_element(),
            section(
                "Initials",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(
                        "Initials come from the first and last words of the name.",
                        cx,
                    ))
                    .child(sizes("initials", |avatar| avatar.name("Ada Lovelace"))),
            )
            .into_any_element(),
            section("Icon", sizes("icon", |avatar| avatar.icon(IconName::User))).into_any_element(),
            section(
                "Group",
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        AvatarGroup::new("team-images").children(
                            PORTRAITS
                                .into_iter()
                                .enumerate()
                                .map(|(index, (from, to))| {
                                    Avatar::new(format!("portrait-{from}"))
                                        .src(portrait(from, to))
                                        .name(PEOPLE[index].1)
                                }),
                        ),
                    )
                    .child(AvatarGroup::new("team").children(people()))
                    .child(AvatarGroup::new("team-limited").children(people()).limit(3))
                    .child(
                        AvatarGroup::new("team-small")
                            .children(people())
                            .size(AvatarSize::Sm)
                            .limit(4),
                    ),
            )
            .into_any_element(),
        ])
    }
}

/// Gradient stops for the group of portraits.
const PORTRAITS: [(&str, &str); 4] = [
    ("#6366f1", "#ec4899"),
    ("#0ea5e9", "#22c55e"),
    ("#f59e0b", "#ef4444"),
    ("#14b8a6", "#8b5cf6"),
];
