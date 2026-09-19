use gpui_kit::{
    AnyView, App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, Styled as _,
    Subscription, Window, actions,
    base::{TextSelection, TextSelectionLayer, TooltipOverlay},
    div,
};

use crate::{ActiveTheme as _, Theme, TooltipHost};

actions!(gpui_cn_root, [Tab, TabPrev, Copy]);

const CONTEXT: &str = "GpuiCnRoot";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Tab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", TabPrev, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
    ]);
}

/// The first view in a window that gpui-cn owns.
///
/// Root paints the window surface from the theme, sets the rem size the
/// rem-based scale resolves against, moves keyboard focus on Tab and
/// Shift-Tab (staying inside an active focus trap), copies the window text
/// selection, and hosts the overlay layers gpui-cn components render into.
///
/// Root holds keyboard focus itself whenever nothing else does, so the
/// first Tab in a fresh window reaches its handler and lands on the first
/// tab stop. Without that, GPUI dispatches keys only along the focused
/// element's ancestors, and a window where nothing is focused would never
/// see Tab at all. The same recovery runs when the focused element is
/// removed; an owner that keeps a focused [`FocusHandle`] alive while
/// hiding its element should blur the window itself.
///
/// A window that already has another root (for example
/// `gpui_component::Root`) does not need this one; gpui-cn components still
/// work there and register nothing. Construct it after [`crate::init`].
pub struct Root {
    view: AnyView,
    focus_handle: FocusHandle,
    tooltip_overlay: Entity<TooltipOverlay>,
    _appearance_observer: Subscription,
}

impl Root {
    /// Wraps `view` as the window content.
    pub fn new(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tooltip_overlay =
            cx.new(|_| TooltipOverlay::new().render_with(crate::tooltip::render_surface));
        TooltipHost::register(window, &tooltip_overlay, cx);
        Theme::sync_system_appearance(Some(window), cx);
        let appearance_observer = window.observe_window_appearance(|window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });
        Self {
            view: view.into(),
            focus_handle: cx.focus_handle(),
            tooltip_overlay,
            _appearance_observer: appearance_observer,
        }
    }

    /// The focus handle Root holds when nothing else is focused.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// The tooltip overlay this root renders.
    pub fn tooltip_overlay(&self) -> &Entity<TooltipOverlay> {
        &self.tooltip_overlay
    }

    fn on_action_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        move_focus(window, cx, Window::focus_next);
    }

    fn on_action_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
        move_focus(window, cx, Window::focus_prev);
    }

    fn on_action_copy(&mut self, _: &Copy, window: &mut Window, cx: &mut Context<Self>) {
        let text = TextSelection::selected_text(window, cx);
        let text = text.trim();
        if text.is_empty() {
            cx.propagate();
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
    }
}

/// Moves focus one step, and keeps it inside the active focus trap.
///
/// When a step leaves the trap, focus keeps stepping in the same direction
/// until it is back inside, which wraps from the trap's last focusable to its
/// first (or the reverse). The attempt cap guards a trap with nothing
/// focusable in it.
fn move_focus(window: &mut Window, cx: &mut App, step: fn(&mut Window, &mut App)) {
    let Some(trap) = gpui_kit::base::active_focus_trap(window, cx) else {
        step(window, cx);
        return;
    };
    const MAX_ATTEMPTS: usize = 100;
    let before = window.focused(cx);
    step(window, cx);
    let mut attempts = 0;
    while !trap.contains_focused(window, cx) && attempts < MAX_ATTEMPTS {
        step(window, cx);
        attempts += 1;
        if window.focused(cx) == before {
            break;
        }
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(Theme::global(cx).ui_font_size);
        if window.focused(cx).is_none() {
            window.focus(&self.focus_handle, cx);
        }
        let tokens = cx.theme();
        let font_family = tokens.font_family().clone();
        let background = tokens.background();
        let foreground = tokens.foreground();
        div()
            .id("gpui-cn-root")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_action_tab))
            .on_action(cx.listener(Self::on_action_tab_prev))
            .on_action(cx.listener(Self::on_action_copy))
            .relative()
            .size_full()
            .font_family(font_family)
            .bg(background)
            .text_color(foreground)
            .child(TextSelectionLayer)
            .child(self.view.clone())
            .child(self.tooltip_overlay.clone())
    }
}
