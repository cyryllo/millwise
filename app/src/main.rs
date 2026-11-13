// Millwise — CAM dla frezarek CNC 3-osiowych.
// Licencja: GPL-3.0-or-later

mod app;
mod sim_panel;
mod icons;
mod toolbar;
mod ui_machines;
mod storage;
mod ui_ops;
mod ui_tools;
mod doc;
mod history;
mod view2d;
mod view3d;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // parametry wiersza poleceń (bez okna) — m.in. do sprawdzania paczek
    if let Some(a) = std::env::args().nth(1) {
        match a.as_str() {
            "--version" | "-V" => {
                println!("Millwise {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "Millwise {} — CAM dla frezarek CNC\n\nUżycie: millwise [plik.dxf | projekt.millwise]\n  --version   wersja programu\n  --help      ta pomoc",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            _ => {}
        }
    }

    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Millwise")
            .with_app_id("millwise")
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: include_bytes!("../../assets/icon/millwise-256.rgba").to_vec(),
                width: 256,
                height: 256,
            }))
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Millwise",
        options,
        Box::new(move |cc| Ok(Box::new(app::CamApp::new(cc, initial_file)))),
    )
}
