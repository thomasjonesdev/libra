//! Compiles every fixture through libra-codegen at build time — the
//! release-engine half of the §6.4 no-drift check. A codegen error
//! becomes a build error carrying the .lx span.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=fixtures");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    for entry in std::fs::read_dir("fixtures").expect("fixtures dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("lx") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&path).expect("read fixture");
        let ast = libra_markup::parse(&src)
            .unwrap_or_else(|e| panic!("{}:{e}", path.display()));
        let code = libra_codegen::generate(
            &ast,
            &libra_codegen::Config {
                model: "crate::TestModel".into(),
                msg: "crate::TestMsg".into(),
                fn_name: format!("build_{name}"),
                source: path.display().to_string(),
            },
        )
        .unwrap_or_else(|e| panic!("{}:{e}", path.display()));
        std::fs::write(Path::new(&out_dir).join(format!("{name}.rs")), code)
            .expect("write generated view");
    }
}
