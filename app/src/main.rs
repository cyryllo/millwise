// Millwise — CAM dla frezarek CNC 3-osiowych.
// Licencja: GPL-3.0-or-later

mod app;
mod sim_panel;
mod icons;
mod toolbar;
mod ui_machines;
mod storage;
mod ui_kit;
mod ui_ops;
mod ui_tools;
mod doc;
mod history;
mod view2d;
mod view3d;

use std::sync::atomic::{AtomicBool, Ordering};

/// Ustawiane przy pierwszej narysowanej klatce — wiadomo wtedy, że grafika działa.
pub static FIRST_FRAME: AtomicBool = AtomicBool::new(false);

/// Zmienna środowiskowa dziecka uruchomionego ponownie z OpenGL (żeby nie zapętlić).
const FALLBACK_ENV: &str = "MILLWISE_GL_FALLBACK";

/// Wybór grafiki (wgpu): `--gl` / `--vulkan`, potem zapamiętany wybór, potem
/// sterownik NVIDIA → OpenGL (Vulkan na NVIDIA pod Linuksem często nie otwiera okna).
/// Zmienna WGPU_BACKEND ustawiona przez użytkownika ma pierwszeństwo.
fn choose_backend(args: &[String]) {
    if std::env::var_os("WGPU_BACKEND").is_some() {
        return;
    }
    let forced = if args.iter().any(|a| a == "--gl") {
        Some("gl")
    } else if args.iter().any(|a| a == "--vulkan") {
        Some("vulkan")
    } else {
        None
    };
    let chosen = forced.or_else(|| storage::gl_remembered().then_some("gl")).or_else(|| {
        std::path::Path::new("/proc/driver/nvidia/version").exists().then_some("gl")
    });
    if let Some(b) = chosen {
        // SAFETY: przed uruchomieniem jakichkolwiek wątków
        unsafe { std::env::set_var("WGPU_BACKEND", b) };
    }
}

/// Ponowne uruchomienie programu z OpenGL, gdy domyślna grafika nie otworzyła okna.
fn rerun_with_gl() -> Option<i32> {
    if std::env::var_os(FALLBACK_ENV).is_some() || std::env::var("WGPU_BACKEND").is_ok_and(|b| b.eq_ignore_ascii_case("gl")) {
        return None;
    }
    // w AppImage uruchamiamy sam plik AppImage, nie program z tymczasowego katalogu
    let exe = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).or_else(|| std::env::current_exe().ok())?;
    eprintln!("Millwise: grafika domyślna nie działa — ponowne uruchomienie z OpenGL (zapamiętane na kolejne starty).");
    let status = std::process::Command::new(exe)
        .args(std::env::args().skip(1))
        .env("WGPU_BACKEND", "gl")
        .env(FALLBACK_ENV, "1")
        .status()
        .ok()?;
    Some(status.code().unwrap_or(1))
}

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
                    "Millwise {} — CAM dla frezarek CNC\n\nUżycie: millwise [opcje] [plik.dxf | projekt.millwise] [kolejne pliki…]\n  (pierwszy plik jest otwierany, kolejne dochodzą do projektu)\n  --gl        grafika przez OpenGL (np. gdy okno się nie otwiera)\n  --vulkan    grafika przez Vulkan\n  --version   wersja programu\n  --help      ta pomoc",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            _ => {}
        }
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    choose_backend(&args);
    let files: Vec<std::path::PathBuf> =
        args.iter().filter(|a| !a.starts_with("--")).map(std::path::PathBuf::from).collect();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Millwise")
            .with_app_id("millwise")
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: include_bytes!("../../assets/icon/millwise-256.rgba").to_vec(),
                width: 256,
                height: 256,
            }))
            .with_inner_size([1600.0, 1000.0])
            // zrzuty ekranu (MILLWISE_SCREENSHOT) w stałym rozmiarze, nie na cały monitor
            .with_maximized(std::env::var_os("MILLWISE_SCREENSHOT").is_none())
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native("Millwise", options, Box::new(move |cc| Ok(Box::new(app::CamApp::new(cc, files)))))
    }));
    let failed_before_window = !FIRST_FRAME.load(Ordering::SeqCst) && !matches!(result, Ok(Ok(())));
    if failed_before_window {
        match &result {
            Ok(Err(e)) => eprintln!("Millwise: nie udało się otworzyć okna: {e}"),
            Err(_) => eprintln!("Millwise: błąd przy otwieraniu okna"),
            Ok(Ok(())) => {}
        }
        if let Some(code) = rerun_with_gl() {
            if code == 0 {
                storage::remember_gl();
            }
            std::process::exit(code);
        }
    }
    match result {
        Ok(r) => r,
        Err(p) => std::panic::resume_unwind(p),
    }
}
