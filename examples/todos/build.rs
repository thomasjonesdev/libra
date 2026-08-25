fn main() {
    libra_build::compile_app(&libra_build::AppBuildConfig {
        manifest_dir: std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        view: "views/todos.lx".into(),
        state: "crate::Todos".into(),
        action: "crate::Action".into(),
    });
}
