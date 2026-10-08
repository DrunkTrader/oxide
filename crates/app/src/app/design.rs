use super::*;

pub(super) const ACCENT: Color32 = Color32::from_rgb(184, 109, 75);

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
    pub accent_text: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                canvas: Color32::from_rgb(24, 25, 27),
                rail: Color32::from_rgb(29, 30, 32),
                surface: Color32::from_rgb(34, 35, 38),
                raised: Color32::from_rgb(39, 40, 43),
                hover: Color32::from_rgb(46, 47, 50),
                border: Color32::from_rgb(57, 58, 62),
                text: Color32::from_rgb(236, 234, 231),
                muted: Color32::from_rgb(155, 156, 160),
                accent_text: Color32::from_rgb(219, 149, 115),
                accent_soft: Color32::from_rgb(61, 43, 35),
                danger: Color32::from_rgb(225, 131, 127),
            }
        } else {
            Self {
                canvas: Color32::from_rgb(250, 249, 247),
                rail: Color32::from_rgb(241, 240, 237),
                surface: Color32::from_rgb(255, 254, 252),
                raised: Color32::from_rgb(255, 254, 252),
                hover: Color32::from_rgb(231, 229, 225),
                border: Color32::from_rgb(218, 216, 211),
                text: Color32::from_rgb(43, 43, 45),
                muted: Color32::from_rgb(112, 112, 117),
                accent_text: Color32::from_rgb(150, 75, 44),
                accent_soft: Color32::from_rgb(244, 226, 215),
                danger: Color32::from_rgb(172, 60, 57),
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
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size = egui::vec2(32.0, 32.0);
        style.spacing.window_margin = egui::Margin::same(24);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.extra_text_line_spacing = 3.0;
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(24.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(13.0)),
            (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(13.0)),
        ]
        .into();
        style.animation_time = 0.12;
        let v = &mut style.visuals;
        v.override_text_color = None;
        v.weak_text_color = Some(p.muted);
        v.panel_fill = p.canvas;
        v.window_fill = p.raised;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.window_corner_radius = egui::CornerRadius::same(10);
        v.menu_corner_radius = egui::CornerRadius::same(8);
        v.window_shadow = egui::epaint::Shadow {
            offset: [0, 10],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if v.dark_mode { 72 } else { 24 }),
        };
        v.popup_shadow = v.window_shadow;
        v.window_highlight_topmost = false;
        v.faint_bg_color = p.surface;
        v.extreme_bg_color = p.canvas;
        v.text_edit_bg_color = Some(p.surface);
        v.code_bg_color = p.surface;
        v.hyperlink_color = p.accent_text;
        v.warn_fg_color = p.accent_text;
        v.error_fg_color = p.danger;
        v.selection.bg_fill = p.accent_soft;
        v.selection.stroke = egui::Stroke::new(1.0, p.text);
        v.text_cursor.stroke = egui::Stroke::new(2.0, p.accent_text);
        let widgets = &mut v.widgets;
        for (widget, fill, stroke) in [
            (&mut widgets.noninteractive, p.canvas, p.border),
            (&mut widgets.inactive, p.surface, p.border),
            (&mut widgets.hovered, p.hover, p.muted),
            (&mut widgets.active, p.accent_soft, p.accent_text),
            (&mut widgets.open, p.hover, p.border),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = egui::Stroke::new(1.0, stroke);
            widget.fg_stroke = egui::Stroke::new(1.0, p.text);
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.expansion = 0.0;
        }
    });
}

pub(super) fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE))
            .fill(ACCENT)
            .stroke(egui::Stroke::NONE),
    )
}

pub(super) fn quiet(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).frame_when_inactive(false))
}

pub(super) fn section(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.label(RichText::new(title).size(22.0).strong());
    if !description.is_empty() {
        ui.label(
            RichText::new(description)
                .size(13.0)
                .color(Palette::of(ui).muted),
        );
    }
}

pub(super) fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(11.0).color(Palette::of(ui).muted));
}

pub(super) fn panel_frame(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::new().fill(fill).inner_margin(margin)
}

pub(super) fn card(ui: &egui::Ui) -> egui::Frame {
    let p = Palette::of(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(8)
        .inner_margin(14)
}
