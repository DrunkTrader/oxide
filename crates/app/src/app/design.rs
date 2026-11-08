use super::*;
use egui::emath::easing;

/// Apple-style neutrals with one violet accent taken from the Oxide mark.
/// Colour carries meaning (accent = Oxide, green = saved/indexed, red = needs attention), never decoration.
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
                canvas: Color32::from_rgb(18, 18, 20),
                rail: Color32::from_rgb(25, 25, 28),
                surface: Color32::from_rgb(32, 32, 36),
                raised: Color32::from_rgb(40, 40, 45),
                hover: Color32::from_rgb(48, 48, 54),
                border: Color32::from_rgb(56, 56, 62),
                text: Color32::from_rgb(245, 245, 247),
                muted: Color32::from_rgb(152, 152, 159),
                accent: Color32::from_rgb(139, 124, 255),
                accent_text: Color32::from_rgb(184, 173, 255),
                accent_soft: Color32::from_rgb(45, 40, 74),
                danger: Color32::from_rgb(255, 69, 58),
                success: Color32::from_rgb(48, 209, 88),
            }
        } else {
            Self {
                canvas: Color32::from_rgb(245, 245, 247),
                rail: Color32::from_rgb(236, 236, 240),
                surface: Color32::from_rgb(255, 255, 255),
                raised: Color32::from_rgb(255, 255, 255),
                hover: Color32::from_rgb(228, 228, 234),
                border: Color32::from_rgb(216, 216, 222),
                text: Color32::from_rgb(29, 29, 31),
                muted: Color32::from_rgb(110, 110, 115),
                accent: Color32::from_rgb(108, 84, 230),
                accent_text: Color32::from_rgb(86, 62, 206),
                accent_soft: Color32::from_rgb(236, 232, 255),
                danger: Color32::from_rgb(215, 0, 21),
                success: Color32::from_rgb(36, 138, 61),
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
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(26.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(13.5)),
            (egui::TextStyle::Small, egui::FontId::proportional(11.5)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(13.5)),
        ]
        .into();
        style.animation_time = 0.22;
        let v = &mut style.visuals;
        v.override_text_color = None;
        v.weak_text_color = Some(p.muted);
        v.panel_fill = p.canvas;
        v.window_fill = p.raised;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.window_corner_radius = egui::CornerRadius::same(16);
        v.menu_corner_radius = egui::CornerRadius::same(12);
        v.window_shadow = egui::epaint::Shadow {
            offset: [0, 24],
            blur: 64,
            spread: 0,
            color: Color32::from_black_alpha(if v.dark_mode { 140 } else { 40 }),
        };
        v.popup_shadow = egui::epaint::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(if v.dark_mode { 100 } else { 26 }),
        };
        v.window_highlight_topmost = false;
        v.faint_bg_color = p.surface;
        v.extreme_bg_color = p.surface;
        v.text_edit_bg_color = Some(p.surface);
        v.code_bg_color = p.raised;
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
            widget.corner_radius = egui::CornerRadius::same(9);
            widget.expansion = 0.0;
        }
    });
}

// ───────────── Motion primitives ─────────────

/// Seconds since `key` last changed for this `salt` (a view switch, a note switch, a tab change).
fn since_change(ctx: &egui::Context, salt: &str, key: egui::Id) -> f64 {
    let slot = egui::Id::new(("entrance", salt));
    let now = ctx.input(|input| input.time);
    let start = ctx.data_mut(|data| {
        let entry = data.get_temp_mut_or_insert_with(slot, || (key, now));
        if entry.0 != key {
            *entry = (key, now);
        }
        entry.1
    });
    now - start
}

/// Content fades and rises into place whenever `key` changes.
pub(super) fn entrance(ui: &mut egui::Ui, salt: &str, key: impl std::hash::Hash + std::fmt::Debug) {
    let ctx = ui.ctx().clone();
    let t = (since_change(&ctx, salt, egui::Id::new(key)) / 0.42).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    let eased = easing::cubic_out(t);
    ui.multiply_opacity(eased);
    ui.add_space((1.0 - eased) * 22.0);
}

/// Staggered entrance of the n-th item of a collection: 0..1, items 40 ms apart, measured
/// from the last change of `salt`/`key` (call `entrance` with the same salt and key first).
pub(super) fn stagger(
    ui: &egui::Ui,
    salt: &str,
    key: impl std::hash::Hash + std::fmt::Debug,
    index: usize,
) -> f32 {
    let ctx = ui.ctx();
    let since = since_change(ctx, salt, egui::Id::new(key)) - 0.04 * index.min(24) as f64;
    let t = (since / 0.38).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    easing::cubic_out(t)
}

/// Eased 0..1 hover progress and press state for the widget `ui` adds next (last frame's response).
fn hover_progress(ui: &egui::Ui, id: egui::Id) -> (f32, bool) {
    let r = ui.ctx().read_response(id);
    let hovered = r.as_ref().is_some_and(|r| r.hovered());
    let pressed = r.is_some_and(|r| r.is_pointer_button_down_on());
    (
        ui.ctx().animate_bool_responsive(id.with("hover"), hovered),
        pressed,
    )
}

fn filled(ui: &mut egui::Ui, text: &str, base: Color32) -> egui::Response {
    let (t, pressed) = hover_progress(ui, ui.next_auto_id());
    let fill = if pressed {
        base.lerp_to_gamma(Color32::BLACK, 0.12)
    } else {
        base.lerp_to_gamma(Color32::WHITE, 0.12 * t)
    };
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(fill)
            .stroke(egui::Stroke::NONE)
            .corner_radius(10)
            .min_size(egui::vec2(0.0, 34.0)),
    )
}

pub(super) fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    filled(ui, text, Palette::of(ui).accent)
}

pub(super) fn destructive(ui: &mut egui::Ui, text: &str) -> egui::Response {
    filled(ui, text, Palette::of(ui).danger)
}

pub(super) fn quiet(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(text)
            .frame_when_inactive(false)
            .corner_radius(9),
    )
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

/// A smaller heading for a group inside a page or the settings sheet.
pub(super) fn group_title(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.label(RichText::new(title).size(16.0).strong());
    if !description.is_empty() {
        ui.label(
            RichText::new(description)
                .size(12.5)
                .color(Palette::of(ui).muted),
        );
    }
    ui.add_space(6.0);
}

pub(super) fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(11.5).color(Palette::of(ui).muted));
}

pub(super) fn panel_frame(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::new().fill(fill).inner_margin(margin)
}

/// A rounded group of settings rows (an inset grouped list).
pub(super) fn form_group<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = Palette::of(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// One settings row: label (and optional hint) on the left, control on the right.
pub(super) fn form_row(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    control: impl FnOnce(&mut egui::Ui),
) {
    let p = Palette::of(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(40.0);
        ui.vertical(|ui| {
            ui.add_space(if hint.is_empty() { 10.0 } else { 3.0 });
            ui.label(RichText::new(label).size(14.0));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(12.0).color(p.muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
}

/// Segmented control with a sliding selection; returns true when the selection changed.
pub(super) fn segmented(
    ui: &mut egui::Ui,
    id: &str,
    options: &[&str],
    selected: &mut usize,
) -> bool {
    let p = Palette::of(ui);
    let font = egui::FontId::proportional(13.0);
    let widths: Vec<f32> = options
        .iter()
        .map(|s| {
            ui.painter()
                .layout_no_wrap((*s).to_owned(), font.clone(), p.text)
                .size()
                .x
                + 28.0
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total, 32.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 10.0, p.hover);
    let mut x = rect.left() + 3.0;
    let mut rects = Vec::new();
    for w in &widths {
        rects.push(egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + 3.0),
            egui::vec2(*w, 26.0),
        ));
        x += w;
    }
    let mut changed = false;
    for (i, r) in rects.iter().enumerate() {
        let response = ui.interact(*r, ui.id().with((id, i)), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                true,
                *selected == i,
                options[i],
            )
        });
        if response.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
    }
    let ctx = ui.ctx();
    let target = rects[(*selected).min(rects.len() - 1)];
    let x0 = ctx.animate_value_with_time(ui.id().with((id, "x")), target.left(), 0.22);
    let w0 = ctx.animate_value_with_time(ui.id().with((id, "w")), target.width(), 0.22);
    let pill = egui::Rect::from_min_size(
        egui::pos2(x0, target.top()),
        egui::vec2(w0, target.height()),
    );
    let painter = ui.painter();
    painter.rect_filled(
        pill.translate(egui::vec2(0.0, 1.0)),
        8.0,
        Color32::from_black_alpha(36),
    );
    painter.rect_filled(
        pill,
        8.0,
        if ui.visuals().dark_mode {
            Palette::new(true).border
        } else {
            Color32::WHITE
        },
    );
    for (i, (r, s)) in rects.iter().zip(options).enumerate() {
        let color = if i == *selected { p.text } else { p.muted };
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *s,
            font.clone(),
            color,
        );
    }
    changed
}

/// Search field: a pill with a magnifier and a focus ring that glows in.
pub(super) fn search_field(
    ui: &mut egui::Ui,
    text: &mut String,
    width: f32,
    hint: &str,
) -> egui::Response {
    let p = Palette::of(ui);
    let id = egui::Id::new("search");
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let f = ui.ctx().animate_bool_responsive(id.with("focus"), focused);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 38.0), egui::Sense::hover());
    let painter = ui.painter();
    if f > 0.0 {
        painter.rect_filled(
            rect.expand(3.5 * f),
            23.0,
            p.accent.gamma_multiply(0.25 * f),
        );
    }
    painter.rect_filled(rect, 19.0, p.surface);
    painter.rect_stroke(
        rect,
        19.0,
        egui::Stroke::new(1.0, p.border.lerp_to_gamma(p.accent, f)),
        egui::StrokeKind::Inside,
    );
    let c = egui::pos2(rect.left() + 20.0, rect.center().y - 1.0);
    painter.circle_stroke(c, 5.5, egui::Stroke::new(1.6, p.muted));
    painter.line_segment(
        [c + egui::vec2(4.0, 4.0), c + egui::vec2(8.0, 8.0)],
        egui::Stroke::new(1.6, p.muted),
    );
    let inner = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 36.0, rect.top()),
        egui::pos2(rect.right() - 14.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .desired_width(inner.width())
            .font(egui::FontId::proportional(14.0)),
    )
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
    let shadow_alpha = if ui.visuals().dark_mode { 130.0 } else { 34.0 };
    let inner = egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(
            1.0,
            p.border.lerp_to_gamma(p.accent, 0.55 * t),
        ))
        .corner_radius(14)
        .inner_margin(10)
        .shadow(egui::epaint::Shadow {
            offset: [0, (2.0 + 10.0 * t) as i8],
            blur: (8.0 + 22.0 * t) as u8,
            spread: 0,
            color: Color32::from_black_alpha((shadow_alpha * (0.35 + 0.65 * t)) as u8),
        })
        .show(ui, add_contents);
    let now = inner.response.contains_pointer();
    ui.ctx().data_mut(|d| d.insert_temp(id, now));
    inner.inner
}

/// Loading placeholder: a soft band of light sweeps across the rect.
pub(super) fn shimmer(ui: &egui::Ui, rect: egui::Rect) {
    let p = Palette::of(ui);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 10.0, p.hover);
    let time = ui.input(|i| i.time) as f32;
    let x = rect.left() - rect.width() * 0.4 + (time * 0.8).fract() * rect.width() * 1.8;
    for k in 0..12 {
        let a = (1.0 - (k as f32 - 6.0).abs() / 6.0) * 0.07;
        let band = egui::Rect::from_min_size(
            egui::pos2(x + k as f32 * 8.0, rect.top()),
            egui::vec2(8.0, rect.height()),
        );
        painter.rect_filled(band, 0.0, Color32::WHITE.gamma_multiply(a));
    }
    ui.ctx().request_repaint();
}

pub(super) fn nav_item(ui: &mut egui::Ui, text: &str, selected: bool) -> bool {
    list_row(ui, text, "", selected)
}

/// List row with an animated fill. A selected row reports its rect so that
/// `nav_indicator` can slide one accent bar between rows.
pub(super) fn list_row(ui: &mut egui::Ui, text: &str, detail: &str, selected: bool) -> bool {
    let p = Palette::of(ui);
    let id = ui.id().with(("nav", text));
    let height = if detail.is_empty() { 34.0 } else { 54.0 };
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
    painter.rect_filled(rect, 9.0, fill);
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            9.0,
            egui::Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    if selected {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("nav-selected"), rect));
    }
    let color = p.text.lerp_to_gamma(p.accent_text, sel);
    let clip = ui.painter_at(rect.shrink2(egui::vec2(14.0, 0.0)));
    let left = clip.clip_rect().left() + 3.0 * sel;
    let title = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(13.5), color);
    if detail.is_empty() {
        let y = rect.center().y - title.size().y / 2.0;
        clip.galley(egui::pos2(left, y), title, color);
    } else {
        let sub =
            painter.layout_no_wrap(detail.to_owned(), egui::FontId::proportional(12.0), p.muted);
        clip.galley(egui::pos2(left, rect.top() + 9.0), title, color);
        clip.galley(egui::pos2(left, rect.top() + 30.0), sub, p.muted);
    }
    response.clicked()
}

/// Draws the accent bar for the list drawn since the last call, sliding it to the selected row.
pub(super) fn nav_indicator(ui: &mut egui::Ui, salt: &str) {
    let p = Palette::of(ui);
    let ctx = ui.ctx().clone();
    let key = egui::Id::new("nav-selected");
    let Some(rect) = ctx.data_mut(|d| {
        let r = d.get_temp::<egui::Rect>(key);
        d.remove::<egui::Rect>(key);
        r
    }) else {
        return;
    };
    let id = egui::Id::new(("nav-indicator", salt));
    let y = ctx.animate_value_with_time(id.with("y"), rect.center().y, 0.26);
    let h = ctx.animate_value_with_time(id.with("h"), rect.height() * 0.5, 0.26);
    ui.painter().rect_filled(
        egui::Rect::from_center_size(egui::pos2(rect.left() + 4.0, y), egui::vec2(3.5, h)),
        2.0,
        p.accent,
    );
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

/// A notice as a floating toast that springs up from the bottom; returns true once dismissed
/// (by the button, or on its own after six seconds).
pub(super) fn toast(ctx: &egui::Context, message: &str, age: f32) -> bool {
    let dark = ctx.global_style().visuals.dark_mode;
    let p = Palette::new(dark);
    let rise = easing::back_out((age / 0.35).clamp(0.0, 1.0));
    let fade = 1.0 - ((age - 5.6) / 0.4).clamp(0.0, 1.0);
    let mut dismissed = age > 6.0;
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("oxide-toast"))
        .order(egui::Order::Tooltip)
        .fixed_pos(egui::pos2(
            screen.center().x,
            screen.bottom() - 64.0 + (1.0 - rise) * 40.0,
        ))
        .pivot(egui::Align2::CENTER_BOTTOM)
        .show(ctx, |ui| {
            ui.multiply_opacity(rise.clamp(0.0, 1.0) * fade);
            egui::Frame::new()
                .fill(p.raised)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(22)
                .inner_margin(egui::Margin::symmetric(18, 8))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 10],
                    blur: 30,
                    spread: 0,
                    color: Color32::from_black_alpha(if dark { 120 } else { 36 }),
                })
                .show(ui, |ui| {
                    ui.set_max_width(680.0);
                    ui.horizontal(|ui| {
                        status_dot(ui, p.accent, false);
                        ui.add(egui::Label::new(RichText::new(message).size(13.5)).wrap());
                        if quiet(ui, "Dismiss").clicked() {
                            dismissed = true;
                        }
                    });
                });
        });
    if !dismissed {
        ctx.request_repaint();
    }
    dismissed
}
