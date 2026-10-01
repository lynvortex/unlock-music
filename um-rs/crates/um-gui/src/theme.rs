//! 主题与通用控件。
//!
//! 配色取自 daisyUI 的 `balanced` 主题，圆角统一 0.5rem。

use eframe::egui;
use egui::{Color32, CornerRadius, FontId, Rect, RichText, Stroke, Vec2};

// —— 基础色 ——
/// base-100
pub const BASE_100: Color32 = Color32::from_rgb(0xFA, 0xFA, 0xFA);
/// base-200
pub const BASE_200: Color32 = Color32::from_rgb(0xF4, 0xF4, 0xF5);
/// base-300
pub const BASE_300: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE7);
/// base-content
pub const BASE_CONTENT: Color32 = Color32::from_rgb(0x18, 0x18, 0x1B);

// —— 语义色 ——
pub const PRIMARY: Color32 = Color32::from_rgb(0x0D, 0x94, 0x88);
pub const PRIMARY_CONTENT: Color32 = Color32::from_rgb(0xF0, 0xFD, 0xFA);
pub const ACCENT: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
pub const INFO: Color32 = Color32::from_rgb(0x1D, 0x4E, 0xD8);
pub const SUCCESS: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
pub const WARNING: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
pub const ERROR: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);

/// 0.5rem
pub const RADIUS: u8 = 8;

/// 弱化文字用（对应 tailwind 的 opacity-50 一档）
pub fn weak_text(text: impl Into<String>) -> RichText {
    RichText::new(text).color(BASE_CONTENT.gamma_multiply(0.55))
}

/// 把主题应用到 egui。
pub fn install(ctx: &egui::Context) {
    // 主题固定为浅色，不跟随系统
    ctx.set_theme(egui::ThemePreference::Light);

    let mut visuals = egui::Visuals::light();

    visuals.panel_fill = BASE_100;
    visuals.window_fill = BASE_100;
    visuals.faint_bg_color = BASE_200;
    visuals.extreme_bg_color = Color32::WHITE;
    visuals.override_text_color = Some(BASE_CONTENT);
    visuals.window_corner_radius = CornerRadius::same(RADIUS);
    visuals.selection.bg_fill = PRIMARY.gamma_multiply(0.25);
    visuals.selection.stroke = Stroke::new(1.0, PRIMARY);

    let widgets = &mut visuals.widgets;
    for w in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(RADIUS);
        w.bg_stroke = Stroke::new(1.0, BASE_300);
    }

    widgets.noninteractive.bg_fill = BASE_200;
    widgets.noninteractive.weak_bg_fill = BASE_200;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, BASE_CONTENT.gamma_multiply(0.6));

    // 未交互的按钮走浅灰底，hovered/active 逐级加深
    widgets.inactive.bg_fill = BASE_200;
    widgets.inactive.weak_bg_fill = BASE_200;
    widgets.inactive.fg_stroke = Stroke::new(1.0, BASE_CONTENT);

    widgets.hovered.bg_fill = BASE_300;
    widgets.hovered.weak_bg_fill = BASE_300;
    widgets.hovered.fg_stroke = Stroke::new(1.5, BASE_CONTENT);

    widgets.active.bg_fill = BASE_300;
    widgets.active.weak_bg_fill = BASE_300;
    widgets.active.fg_stroke = Stroke::new(1.5, BASE_CONTENT);

    ctx.set_visuals_of(egui::Theme::Light, visuals);

    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size = Vec2::new(0.0, 28.0);
    ctx.set_style_of(egui::Theme::Light, style);
}

/// 卡片外框：白底 + 细边框 + 圆角，对应 daisyUI 的 `card bg-base-100 shadow-sm`。
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BASE_100)
        .stroke(Stroke::new(1.0, BASE_300))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(14.0)
}

/// 分组区块外框，用于设置页/答疑页的内容块。
pub fn section_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BASE_100)
        .stroke(Stroke::new(1.0, BASE_300))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(16.0)
}

/// 实心主按钮（对应 `btn btn-primary`）。
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    let text = RichText::new(text.into()).color(PRIMARY_CONTENT).strong();
    egui::Button::new(text).fill(PRIMARY)
}

/// 实心危险按钮（对应 `btn btn-error`）。
pub fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
    let text = RichText::new(text.into()).color(Color32::WHITE).strong();
    egui::Button::new(text).fill(ERROR)
}

/// 徽章（对应 `badge badge-accent`）。
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(Color32::WHITE).strong());
        });
}

/// 警告条（对应 `alert alert-warning`），返回右侧按钮是否被点击。
pub fn warning_alert(ui: &mut egui::Ui, title: &str, body: &str, action: &str) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .fill(WARNING.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, WARNING.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚠").size(20.0).color(WARNING));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong());
                    ui.label(RichText::new(body).color(BASE_CONTENT.gamma_multiply(0.75)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(primary_button(action)).clicked() {
                        clicked = true;
                    }
                });
            });
        });
    clicked
}

/// 顶部标签页，返回被点击的标签下标。
///
/// 居中排列，当前项带主题色下划线。
pub fn tab_bar(ui: &mut egui::Ui, tabs: &[&str], current: usize) -> Option<usize> {
    const TAB_WIDTH: f32 = 88.0;

    let mut clicked = None;

    // `ui.horizontal` 会占满可用宽度，内部又按左到右排列，
    // 所以要自己算出居中所需的左侧留白，否则整排会贴左。
    let spacing = ui.spacing().item_spacing.x;
    let total_width = TAB_WIDTH * tabs.len() as f32 + spacing * tabs.len().saturating_sub(1) as f32;
    let left_pad = ((ui.available_width() - total_width) * 0.5).max(0.0);

    ui.horizontal(|ui| {
        ui.add_space(left_pad);

        for (index, name) in tabs.iter().enumerate() {
            let selected = index == current;
            let color = if selected {
                PRIMARY
            } else {
                BASE_CONTENT.gamma_multiply(0.6)
            };

            let response = ui.add(
                egui::Button::new(RichText::new(*name).color(color).strong())
                    .frame(false)
                    .min_size(Vec2::new(TAB_WIDTH, 32.0)),
            );

            if selected {
                // 下划线覆盖整个标签宽度，对应 tabs-border 的 tab-active
                let rect = Rect::from_min_max(
                    egui::pos2(response.rect.left(), response.rect.bottom()),
                    egui::pos2(response.rect.right(), response.rect.bottom() + 2.0),
                );
                ui.painter().rect_filled(rect, 1.0, PRIMARY);
            }

            if response.clicked() {
                clicked = Some(index);
            }
        }
    });

    clicked
}

/// 侧栏导航项，返回是否被点击。
pub fn side_nav_item(ui: &mut egui::Ui, text: &str, selected: bool) -> bool {
    let (fill, color) = if selected {
        (PRIMARY.gamma_multiply(0.14), PRIMARY)
    } else {
        (Color32::TRANSPARENT, BASE_CONTENT.gamma_multiply(0.75))
    };

    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).color(color));
        })
        .response;

    response.interact(egui::Sense::click()).clicked()
}

/// 一级标题。
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(FontId::proportional(20.0)).strong());
}

/// 二级标题。
pub fn sub_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(FontId::proportional(16.0)).strong());
}

/// 说明性小字。
pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(weak_text(text).small());
}
