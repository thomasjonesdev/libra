fn main() {
    // Crate-root relative. Must match `app! { view: "../views/starter.lx" }`
    // in src/lib.rs (PLAN-PACKAGE P0 — two bases, one file).
    libra_build::compile_app(&libra_build::AppBuildConfig {
        manifest_dir: std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        view: "views/starter.lx".into(),
        state: "crate::Starter".into(),
        action: "crate::Action".into(),
    });
}
