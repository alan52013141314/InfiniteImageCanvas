use eframe::egui::{self, Color32, Rect, Stroke, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(139, 207, 188);
pub const PANEL: Color32 = Color32::from_rgb(29, 32, 38);
pub const BACKGROUND: Color32 = Color32::from_rgb(20, 23, 28);
pub const MUTED: Color32 = Color32::from_rgb(147, 155, 168);
pub const BORDER: Color32 = Color32::from_rgb(49, 55, 65);

pub fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut(|s| {
        s.spacing.button_padding = Vec2::new(12.0, 7.0);
        s.spacing.item_spacing = Vec2::new(8.0, 10.0);
        s.spacing.interact_size = Vec2::new(36.0, 32.0);
        s.spacing.window_margin = egui::Margin::same(20);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        s.visuals.override_text_color = Some(Color32::from_rgb(224, 229, 237));
        s.visuals.panel_fill = PANEL;
        s.visuals.window_fill = PANEL;
        s.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
        s.visuals.window_corner_radius = egui::CornerRadius::same(10);
        s.visuals.extreme_bg_color = BACKGROUND;
        s.visuals.faint_bg_color = Color32::from_rgb(36, 40, 47);
        s.visuals.selection.bg_fill = Color32::from_rgb(47, 83, 73);
        s.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
        for w in [
            &mut s.visuals.widgets.inactive,
            &mut s.visuals.widgets.hovered,
            &mut s.visuals.widgets.active,
            &mut s.visuals.widgets.open,
        ] {
            w.corner_radius = egui::CornerRadius::same(6);
            w.bg_stroke = Stroke::new(1.0_f32, BORDER);
            w.fg_stroke.color = Color32::from_rgb(224, 229, 237);
        }
        s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(37, 42, 49);
        s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 42, 49);
        s.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(53, 62, 70);
        s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 62, 70);
        s.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(47, 83, 73);
        s.visuals.widgets.active.bg_fill = Color32::from_rgb(47, 83, 73);
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    });
}

pub fn primary(label: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(label.into()).color(PANEL).strong())
        .fill(ACCENT)
        .stroke(Stroke::NONE)
}

pub fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_stroke(
        Rect::from_min_size(rect.min + Vec2::new(2.0, 7.0), Vec2::new(17.0, 17.0)),
        3.0,
        Stroke::new(1.5_f32, MUTED),
        egui::StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + Vec2::new(7.0, 2.0), Vec2::new(17.0, 17.0)),
        3.0,
        ACCENT,
    );
    painter.line_segment(
        [
            rect.min + Vec2::new(11.0, 13.0),
            rect.min + Vec2::new(15.0, 9.0),
        ],
        Stroke::new(1.5_f32, PANEL),
    );
    painter.line_segment(
        [
            rect.min + Vec2::new(15.0, 9.0),
            rect.min + Vec2::new(20.0, 14.0),
        ],
        Stroke::new(1.5_f32, PANEL),
    );
}

pub fn empty_state(painter: &egui::Painter, rect: Rect, language: crate::i18n::Language) {
    let center = rect.center() - Vec2::new(0.0, 28.0);
    let icon = Rect::from_center_size(center - Vec2::new(0.0, 74.0), Vec2::new(48.0, 40.0));
    painter.rect_stroke(
        icon,
        8.0,
        Stroke::new(1.5_f32, MUTED),
        egui::StrokeKind::Inside,
    );
    painter.circle_filled(icon.min + Vec2::new(13.0, 12.0), 3.0, ACCENT);
    painter.line_segment(
        [
            icon.min + Vec2::new(9.0, 31.0),
            icon.min + Vec2::new(22.0, 19.0),
        ],
        Stroke::new(1.5_f32, MUTED),
    );
    painter.line_segment(
        [
            icon.min + Vec2::new(22.0, 19.0),
            icon.min + Vec2::new(40.0, 32.0),
        ],
        Stroke::new(1.5_f32, MUTED),
    );
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        language.text("拖入圖片或資料夾"),
        egui::FontId::proportional(24.0),
        Color32::from_rgb(224, 229, 237),
    );
    painter.text(
        center + Vec2::new(0.0, 39.0),
        egui::Align2::CENTER_CENTER,
        language.text("自由排列，隨時保存。"),
        egui::FontId::proportional(15.0),
        MUTED,
    );
    painter.text(
        center + Vec2::new(0.0, 91.0),
        egui::Align2::CENTER_CENTER,
        language.text("Ctrl + V 貼上圖片     ·     F11 全螢幕"),
        egui::FontId::proportional(13.0),
        MUTED,
    );
}
