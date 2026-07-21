use lapix::Color as LapixColor;
use macroquad::prelude::Color as MqColor;

pub const BG: egui::Color32 = egui::Color32::from_rgb(38, 38, 38);
pub const BG_FAINT: egui::Color32 = egui::Color32::from_rgb(46, 46, 46);
pub const BG_EXTREME: egui::Color32 = egui::Color32::from_rgb(22, 22, 22);

pub const WIDGET: egui::Color32 = egui::Color32::from_rgb(58, 58, 58);
pub const WIDGET_HOVERED: egui::Color32 = egui::Color32::from_rgb(74, 74, 74);
pub const WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(92, 92, 92);

pub const TEXT: egui::Color32 = egui::Color32::from_rgb(222, 222, 222);
pub const STROKE: egui::Color32 = egui::Color32::from_rgb(72, 72, 72);

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 108, 156);

pub const PREVIEW_BG: egui::Color32 = egui::Color32::from_rgb(52, 52, 52);

// --- CANVAS AREA ---

pub const CANVAS_SURROUND: MqColor = MqColor::new(0.102, 0.102, 0.102, 1.); // 26

pub const CHECKER_1: LapixColor = LapixColor::new(224, 224, 224, 255);
pub const CHECKER_2: LapixColor = LapixColor::new(192, 192, 192, 255);

pub const GRID_LINE: MqColor = MqColor::new(1., 1., 1., 0.18);

pub const SPRITESHEET_LINE: MqColor = MqColor::new(1., 1., 1., 0.55);

pub fn invert_rgb(bytes: &mut [u8]) {
    for px in bytes.chunks_exact_mut(4) {
        px[0] = 255 - px[0];
        px[1] = 255 - px[1];
        px[2] = 255 - px[2];
    }
}

pub fn apply_egui_visuals(egui_ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.menu_corner_radius = 2.into();
    visuals.window_corner_radius = 2.into();

    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.faint_bg_color = BG_FAINT;
    visuals.extreme_bg_color = BG_EXTREME;

    visuals.widgets.noninteractive.bg_fill = BG;
    visuals.widgets.noninteractive.weak_bg_fill = BG;
    visuals.widgets.inactive.bg_fill = WIDGET;
    visuals.widgets.inactive.weak_bg_fill = WIDGET;
    visuals.widgets.hovered.bg_fill = WIDGET_HOVERED;
    visuals.widgets.hovered.weak_bg_fill = WIDGET_HOVERED;
    visuals.widgets.active.bg_fill = WIDGET_ACTIVE;
    visuals.widgets.active.weak_bg_fill = WIDGET_ACTIVE;

    visuals.window_stroke = egui::Stroke::new(1., STROKE);
    visuals.selection.bg_fill = ACCENT;
    visuals.override_text_color = Some(TEXT);

    egui_ctx.set_visuals(visuals);
}
