//! Compiles every fixture through libra-codegen at build time — the
//! release-engine half of the §6.4 no-drift check. A codegen error
//! becomes a build error carrying the .lx span.

use libra_markup::{
    filename_to_component_name, parse_file, ComponentDef, ComponentRegistry, LxFile,
};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=fixtures");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let mut components = Vec::new();
    let mut views = Vec::new();

    for entry in std::fs::read_dir("fixtures").expect("fixtures dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("lx") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&path).expect("read fixture");
        match parse_file(&src).unwrap_or_else(|e| panic!("{}:{e}", path.display())) {
            LxFile::Component(cf) => components.push((stem, cf, path)),
            LxFile::View(ast) => views.push((stem, ast, path)),
        }
    }

    let mut registry = ComponentRegistry::new();
    for (stem, cf, path) in &components {
        registry
            .register(ComponentDef {
                name: filename_to_component_name(stem),
                props: cf.props.clone(),
                props_span: cf.props_span,
                body: cf.body.clone(),
                source: path.display().to_string(),
            })
            .unwrap_or_else(|e| panic!("{e}"));
    }
    registry.detect_cycles().unwrap_or_else(|e| panic!("{e}"));

    let cfg_base = libra_codegen::Config {
        state: "crate::TestState".into(),
        action: "crate::TestAction".into(),
        fn_name: String::new(),
        source: String::new(),
    };

    let mut component_code = String::from("// @generated component fns\n");
    for (_, def) in registry.iter() {
        component_code.push_str(
            &libra_codegen::generate_component_fn(def, &cfg_base, &registry)
                .unwrap_or_else(|e| panic!("component codegen: {e}")),
        );
        component_code.push('\n');
    }
    std::fs::write(Path::new(&out_dir).join("components.rs"), component_code).expect("write components");

    for (name, ast, path) in views {
        let code = libra_codegen::generate_with_registry(
            &ast,
            &libra_codegen::Config {
                state: cfg_base.state.clone(),
                action: cfg_base.action.clone(),
                fn_name: format!("build_{name}"),
                source: path.display().to_string(),
            },
            &registry,
        )
        .unwrap_or_else(|e| panic!("{}:{e}", path.display()));
        std::fs::write(Path::new(&out_dir).join(format!("{name}.rs")), code).expect("write view");
    }
}
