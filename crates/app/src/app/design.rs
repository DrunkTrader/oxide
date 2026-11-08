use super::*;
use egui::emath::easing;

/// Graphite neutrals with one warm oxide accent: colour carries meaning, not decoration.
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub canvas: Color32,
    pub rail: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
    pub success: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                canvas: Color32::from_rgb(22, 22, 24),
                rail: Color32::from_rgb(28, 28, 31),
                surface: Color32::from_rgb(36, 36, 39),
                raised: Color32::from_rgb(44, 44, 47),
                hover: Color32::from_rgb(50, 50, 54),
                border: Color32::from_rgb(56, 56, 60),
                text: Color32::from_rgb(245, 245, 247),
                muted: Color32::from_rgb(152, 152, 158),
                accent: Color32::from_rgb(232, 118, 72),
                accent_text: Color32::from_rgb(246, 152, 112),
                accent_soft: Color32::from_rgb(62, 39, 30),
                danger: Color32::from_rgb(255, 110, 102),
                success: Color32::from_rgb(98, 210, 128),
            }
        } else {
            Self {
                canvas: Color32::from_rgb(251, 251, 253),
                rail: Color32::from_rgb(244, 244, 247),
                surface: Color32::from_rgb(255, 255, 255),
                raised: Color32::from_rgb(255, 255, 255),
                hover: Color32::from_rgb(234, 234, 239),
                border: Color32::from_rgb(224, 224, 229),
                text: Color32::from_rgb(29, 29, 31),
                muted: Color32::from_rgb(110, 110, 115),
                accent: Color32::from_rgb(201, 90, 46),
                accent_text: Color32::from_rgb(168, 70, 32),
                accent_soft: Color32::from_rgb(251, 235, 226),
                danger: Color32::from_rgb(208, 46, 38),
                success: Color32::from_rgb(36, 150, 70),
            }
        }
    }

    pub fn of(ui: &egui::Ui) -> Self {
        Self::new(ui.visuals().dark_mode)
    }
}

pub(super) fn apply_theme(context: &egui::Context, config: &Config) {
    context.set_theme(match config.theme.as_str() {
        "light" => egui::ThemePreference::Light,
        "dark" => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    });
    context.all_styles_mut(|style| {
        let p = Palette::new(style.visuals.dark_mode);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 7.0);
        style.spacing.interact_size = egui::vec2(32.0, 32.0);
        style.spacing.window_margin = egui::Margin::same(28);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.extra_text_line_spacing = 4.0;
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(26.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(13.0)),
            (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(13.0)),
        ]
        .into();
        style.animation_time = 0.2;
        let v = &mut style.visuals;
        v.override_text_color = None;
        v.weak_text_color = Some(p.muted);
        v.panel_fill = p.canvas;
        v.window_fill = p.raised;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.window_corner_radius = egui::CornerRadius::same(14);
        v.menu_corner_radius = egui::CornerRadius::same(10);
        v.window_shadow = egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(if v.dark_mode { 110 } else { 30 }),
        };
        v.popup_shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if v.dark_mode { 90 } else { 22 }),
        };
        v.window_highlight_topmost = false;
        v.faint_bg_color = p.surface;
        v.extreme_bg_color = p.canvas;
        v.text_edit_bg_color = Some(p.surface);
        v.code_bg_color = p.surface;
        v.hyperlink_color = p.accent_text;
        v.warn_fg_color = p.accent_text;
        v.error_fg_color = p.danger;
        v.selection.bg_fill = p.accent_soft;
        v.selection.stroke = egui::Stroke::new(1.5, p.accent_text);
        v.text_cursor.stroke = egui::Stroke::new(2.0, p.accent);
        let widgets = &mut v.widgets;
        for (widget, fill, stroke) in [
            (&mut widgets.noninteractive, p.canvas, p.border),
            (&mut widgets.inactive, p.surface, p.border),
            (&mut widgets.hovered, p.hover, p.border),
            (&mut widgets.active, p.accent_soft, p.accent),
            (&mut widgets.open, p.hover, p.border),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = egui::Stroke::new(1.0, stroke);
            widget.fg_stroke = egui::Stroke::new(1.0, p.text);
            widget.corner_radius = egui::CornerRadius::same(8);
            widget.expansion = 0.0;
        }
    });
}

/// Eased 0..1 hover progress for the widget `ui` adds next (reads last frame's hover).
fn hover_progress(ui: &egui::Ui, id: egui::Id) -> f32 {
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    ui.ctx().animate_bool_responsive(id.with("hover"), hovered)
}

fn filled(ui: &mut egui::Ui, text: &str, base: Color32) -> egui::Response {
    let t = hover_progress(ui, ui.next_auto_id());
    let fill = base.lerp_to_gamma(Color32::WHITE, 0.12 * t);
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(fill)
            .stroke(egui::Stroke::NONE)
            .corner_radius(9),
    )
}

pub(super) fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    filled(ui, text, Palette::of(ui).accent)
}

pub(super) fn destructive(ui: &mut egui::Ui, text: &str) -> egui::Response {
    filled(ui, text, Palette::of(ui).danger)
}

pub(super) fn quiet(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).frame_when_inactive(false))
}

pub(super) fn section(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.label(RichText::new(title).size(24.0).strong());
    if !description.is_empty() {
        ui.add_space(2.0);
        ui.label(
            RichText::new(description)
                .size(13.5)
                .color(Palette::of(ui).muted),
        );
    }
}

pub(super) fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(11.5).color(Palette::of(ui).muted));
}

pub(super) fn panel_frame(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::new().fill(fill).inner_margin(margin)
}

/// Content fades and rises into place whenever `key` changes (view switch, note switch).
pub(super) fn entrance(ui: &mut egui::Ui, salt: &str, key: impl std::hash::Hash + std::fmt::Debug) {
    let ctx = ui.ctx().clone();
    let slot = egui::Id::new(("entrance", salt));
    let key = egui::Id::new(key);
    let now = ctx.input(|input| input.time);
    let start = ctx.data_mut(|data| {
        let entry = data.get_temp_mut_or_insert_with(slot, || (key, now));
        if entry.0 != key {
            *entry = (key, now);
        }
        entry.1
    });
    let t = ((now - start) / 0.32).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    let eased = easing::cubic_out(t);
    ui.multiply_opacity(eased);
    ui.add_space((1.0 - eased) * 12.0);
}

/// A card that lifts (shadow, accent hairline) under the pointer.
pub(super) fn hover_card<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let p = Palette::of(ui);
    let hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    let t = ui.ctx().animate_bool_responsive(id.with("lift"), hovered);
    let shadow_alpha = if ui.visuals().dark_mode { 120.0 } else { 34.0 };
    let inner = egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(
            1.0,
            p.border.lerp_to_gamma(p.accent, 0.45 * t),
        ))
        .corner_radius(12)
        .inner_margin(10)
        .shadow(egui::epaint::Shadow {
            offset: [0, (2.0 + 8.0 * t) as i8],
            blur: (6.0 + 20.0 * t) as u8,
            spread: 0,
            color: Color32::from_black_alpha((shadow_alpha * (0.35 + 0.65 * t)) as u8),
        })
        .show(ui, add_contents);
    let now = inner.response.contains_pointer();
    ui.ctx().data_mut(|d| d.insert_temp(id, now));
    inner.inner
}

pub(super) fn nav_item(ui: &mut egui::Ui, text: &str, selected: bool) -> bool {
    list_row(ui, text, "", selected)
}

/// List row with an animated fill and an accent pill that grows in when selected.
/// A non-empty `detail` adds a muted second line.
pub(super) fn list_row(ui: &mut egui::Ui, text: &str, detail: &str, selected: bool) -> bool {
    let p = Palette::of(ui);
    let id = ui.id().with(("nav", text));
    let height = if detail.is_empty() { 32.0 } else { 52.0 };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let response = ui.interact(rect, id, egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, text));
    let ctx = ui.ctx();
    let sel = ctx.animate_bool_responsive(id.with("sel"), selected);
    let hov = ctx.animate_bool_responsive(id.with("hov"), response.hovered());
    let fill = Color32::TRANSPARENT
        .lerp_to_gamma(p.hover, hov * (1.0 - sel))
        .lerp_to_gamma(p.accent_soft, sel);
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, fill);
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    if sel > 0.0 {
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 3.0, rect.center().y),
                egui::vec2(3.0, 16.0 * sel),
            ),
            1.5,
            p.accent,
        );
    }
    let color = p.text.lerp_to_gamma(p.accent_text, sel);
    let clip = ui.painter_at(rect.shrink2(egui::vec2(12.0, 0.0)));
    let left = clip.clip_rect().left() + 2.0 * sel;
    let title = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(13.5), color);
    if detail.is_empty() {
        let y = rect.center().y - title.size().y / 2.0;
        clip.galley(egui::pos2(left, y), title, color);
    } else {
        let font = egui::FontId::proportional(12.0);
        let sub = painter.layout_no_wrap(detail.to_owned(), font, p.muted);
        clip.galley(egui::pos2(left, rect.top() + 8.0), title, color);
        clip.galley(egui::pos2(left, rect.top() + 29.0), sub, p.muted);
    }
    response.clicked()
}

/// A soft dot: breathes while `busy`, steady otherwise.
pub(super) fn status_dot(ui: &mut egui::Ui, color: Color32, busy: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
    let alpha = if busy {
        ui.ctx().request_repaint();
        let time = ui.input(|input| input.time) as f32;
        0.45 + 0.55 * (0.5 + 0.5 * (time * 5.0).sin())
    } else {
        1.0
    };
    ui.painter()
        .circle_filled(rect.center(), 3.5, color.gamma_multiply(alpha));
}
