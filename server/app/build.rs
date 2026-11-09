fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        copy_vc_runtime();
    }
    tauri_build::build()
}

/// Copies the Visual C++ runtime to `vcredist/` (git-ignored), from where
/// `tauri.windows.conf.json` puts it next to the app: a clean Windows may not
/// have these DLLs, and without them the app doesn't even open.
fn copy_vc_runtime() {
    let system = std::path::Path::new(&std::env::var("SystemRoot").expect("SystemRoot")).join("System32");
    std::fs::create_dir_all("vcredist").expect("criar vcredist");
    for dll in ["msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll", "vcomp140.dll"] {
        let dest = std::path::Path::new("vcredist").join(dll);
        if !dest.exists() {
            std::fs::copy(system.join(dll), &dest)
                .unwrap_or_else(|e| panic!("{dll} não encontrada: instale o Visual C++ Redistributable ({e})"));
        }
    }
}
