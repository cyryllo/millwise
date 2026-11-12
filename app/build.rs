// Ikona i metadane pliku .exe na Windows.
fn main() {
    println!("cargo:rerun-if-changed=../assets/icon/millwise.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../assets/icon/millwise.ico");
        res.set("ProductName", "Millwise");
        res.set("FileDescription", "Millwise — CAM dla frezarek CNC");
        if let Err(e) = res.compile() {
            println!("cargo:warning=nie udało się dołączyć ikony: {e}");
        }
    }
}
