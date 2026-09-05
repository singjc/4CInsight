//! Shared visual theme for the native and WASM 4C Cohort Explorer clients.
//!
//! Keep visual constants here so the two frontends do not drift apart simply
//! because they run in different execution environments.

use eframe::egui;

pub const APP_TITLE: &str = "4C Cohort Explorer";
pub const APP_SUBTITLE: &str = "RNA merging plus metadata-driven multi-modal cohort analysis";

pub fn apply_4c_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(11.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.tooltip_width = 360.0;
    });

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(22, 24, 29);
    visuals.faint_bg_color = egui::Color32::from_rgb(29, 32, 39);
    visuals.extreme_bg_color = egui::Color32::from_rgb(17, 19, 23);
    visuals.selection.bg_fill = egui::Color32::from_rgb(45, 125, 190);
    visuals.hyperlink_color = egui::Color32::from_rgb(105, 180, 235);
    ctx.set_visuals(visuals);
}

pub fn header_frame(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .corner_radius(10)
        .show(ui, add_contents);
}

pub fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(12)
        .corner_radius(8)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).size(17.0).strong());
            ui.add_space(4.0);
            add_contents(ui);
        });
}
