//! Wspólne elementy wyglądu paneli: ramki działów, tytuły, etykiety i ważne pola.

use eframe::egui::{self, Color32, RichText};

pub const CARD_FILL: Color32 = Color32::from_rgb(35, 38, 43);
pub const CARD_STROKE: Color32 = Color32::from_rgb(52, 57, 64);
/// Kolor etykiet pól (jaśniejsze są wartości).
pub const LABEL: Color32 = Color32::from_gray(150);

/// Ramka działu panelu.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD_FILL)
        .stroke(egui::Stroke::new(1.0, CARD_STROKE))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Ramka wyróżniona kolorem (najważniejsze parametry).
pub fn accent_card<R>(ui: &mut egui::Ui, color: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.10))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.7)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Tytuł działu: małe wersaliki.
pub fn title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).small().strong().color(LABEL));
}

/// Tytuł działu z odstępem nad nim (kolejny dział w tej samej ramce).
pub fn subtitle(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    title(ui, text);
}

/// Etykieta pola w siatce.
pub fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(LABEL));
}

/// Pole liczbowe w siatce (etykieta + wartość).
pub fn num(ui: &mut egui::Ui, text: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> bool {
    label(ui, text);
    let r = ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(3)).changed();
    ui.end_row();
    r
}

/// Ważne pole liczbowe: większa, pogrubiona etykieta i większa wartość.
pub fn big_num(ui: &mut egui::Ui, text: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> bool {
    ui.label(RichText::new(text).size(15.0).strong().color(Color32::WHITE));
    let r = ui
        .scope(|ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::proportional(16.0));
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(3)).changed()
        })
        .inner;
    ui.end_row();
    r
}
