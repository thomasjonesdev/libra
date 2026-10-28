//! `libra new` (PLAN-SDK S3, ARCHITECTURE.md §18.1) — write the
//! `templates/app` starter into a new directory.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_GIT: &str = "https://github.com/thomasjonesdev/libra";

// Embedded inside this crate so `cargo package` can publish `libra new`
// (PLAN-SDK S4). Keep these bytes equal to `templates/app/` — the test
// `embedded_template_matches_templates_app` fails if they drift.
// Not named Cargo.toml: a nested manifest makes `cargo package` omit the
// directory, so `libra new` would ship without a starter (PLAN-SDK S4).
const CARGO_TOML: &str = include_str!("../template/Cargo.toml.tpl");
const BUILD_RS: &str = include_str!("../template/build.rs");
const LIB_RS: &str = include_str!("../template/src/lib.rs");
const MAIN_RS: &str = include_str!("../template/src/main.rs");
const VIEW_LX: &str = include_str!("../template/views/starter.lx");
const LIBRA_TOML: &str = include_str!("../template/libra.toml");
const UNIT_RS: &str = include_str!("../template/tests/unit.rs");
const COMPONENTS_RS: &str = include_str!("../template/tests/components.rs");
const SNAPSHOTS_RS: &str = include_str!("../template/tests/snapshots.rs");
const ICON_PNG: &[u8] = include_bytes!("../template/assets/icon.png");

pub fn new(args: &[String]) -> ExitCode {
    match run(args) {
        Ok(spec) => {
            eprintln!("libra new: wrote {}", spec.dest.display());
            eprintln!("  crate: {}", spec.crate_name);
            match &spec.source {
                Source::Path(p) => eprintln!("  deps:  path {}", p.display()),
                Source::Git(u) => eprintln!("  deps:  git {u}"),
                Source::Registry(v) => eprintln!("  deps:  libra-ui {v}"),
            }
            eprintln!("  next:  cd {} && libra dev", spec.dest.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("libra new: {e}");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug)]
struct Spec {
    dest: PathBuf,
    crate_name: String,
    rust_name: String,
    type_name: String,
    title: String,
    source: Source,
}

#[derive(Debug, Clone)]
enum Source {
    Path(PathBuf),
    Git(String),
    /// crates.io `libra-ui` / `libra-test` / `libra-build` at this version.
    Registry(String),
}

fn run(args: &[String]) -> Result<Spec, String> {
    let spec = parse_new_args(args)?;
    if spec.dest.exists() {
        if !is_empty_dir(&spec.dest) {
            return Err(format!(
                "{} is not empty — pick a new directory or an empty one",
                spec.dest.display()
            ));
        }
    } else {
        std::fs::create_dir_all(&spec.dest).map_err(|e| format!("{}: {e}", spec.dest.display()))?;
    }
    write_app(&spec)?;
    Ok(spec)
}

fn parse_new_args(args: &[String]) -> Result<Spec, String> {
    let mut dest: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut source: Option<Source> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--name" => {
                name = Some(it.next().ok_or("--name needs a crate name")?.clone());
            }
            "--path" => {
                let p = it.next().ok_or("--path needs a Libra checkout")?;
                source = Some(Source::Path(PathBuf::from(p)));
            }
            "--git" => {
                let u = it.next().ok_or("--git needs a repository URL")?;
                source = Some(Source::Git(u.clone()));
            }
            s if s.starts_with("--name=") => {
                name = Some(s.trim_start_matches("--name=").to_string());
            }
            s if s.starts_with("--path=") => {
                source = Some(Source::Path(PathBuf::from(s.trim_start_matches("--path="))));
            }
            s if s.starts_with("--git=") => {
                source = Some(Source::Git(s.trim_start_matches("--git=").to_string()));
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown flag `{s}`"));
            }
            s if dest.is_none() => dest = Some(PathBuf::from(s)),
            other => {
                return Err(format!(
                    "unexpected argument `{other}` — usage: libra new <path> [--name crate] [--path checkout | --git url]"
                ));
            }
        }
    }
    let dest =
        dest.ok_or("usage: libra new <path> [--name crate] [--path checkout | --git url]")?;
    let raw_name = name.unwrap_or_else(|| {
        dest.file_name()
            .and_then(|n| n.to_str())
            .filter(|n| *n != "." && *n != "..")
            .unwrap_or("app")
            .to_string()
    });
    let crate_name = crate_name(&raw_name)?;
    let rust_name = crate_name.replace('-', "_");
    let type_name = pascal_case(&crate_name);
    let title = title_case(&crate_name);
    let source = match source {
        Some(s) => s,
        None => default_source(),
    };
    if let Source::Path(p) = &source {
        if !p.join("crates/libra/Cargo.toml").is_file() {
            return Err(format!(
                "{} is not a Libra checkout (missing crates/libra)",
                p.display()
            ));
        }
    }
    Ok(Spec {
        dest,
        crate_name,
        rust_name,
        type_name,
        title,
        source,
    })
}

fn default_source() -> Source {
    // In a framework checkout, path deps. A published `libra-ui-cli`
    // has no sibling `crates/libra`, so the scaffold uses crates.io.
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    if checkout.join("crates/libra/Cargo.toml").is_file() {
        Source::Path(checkout.canonicalize().unwrap_or(checkout))
    } else {
        Source::Registry(env!("CARGO_PKG_VERSION").into())
    }
}

fn crate_name(raw: &str) -> Result<String, String> {
    let s = raw.trim().replace('_', "-").to_ascii_lowercase();
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    let first = s.chars().next().ok_or("crate name is empty")?;
    if !first.is_ascii_alphabetic() {
        return Err(format!("crate name `{s}` must start with a letter"));
    }
    Ok(s)
}

fn pascal_case(kebab: &str) -> String {
    kebab
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn title_case(kebab: &str) -> String {
    kebab
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_empty_dir(path: &Path) -> bool {
    let Ok(mut entries) = std::fs::read_dir(path) else {
        return false;
    };
    !entries.any(|e| e.map(|e| e.file_name() != ".git").unwrap_or(true))
}

fn write_app(spec: &Spec) -> Result<(), String> {
    let dest = &spec.dest;
    mkdir(dest.join("src"))?;
    mkdir(dest.join("views"))?;
    mkdir(dest.join("assets"))?;
    mkdir(dest.join("tests/snapshots"))?;

    write_text(dest.join("Cargo.toml"), &render_cargo_toml(spec))?;
    write_text(dest.join("build.rs"), &rewrite_build_rs(spec))?;
    write_text(dest.join("src/lib.rs"), &rewrite_lib_rs(spec))?;
    write_text(
        dest.join("src/main.rs"),
        &MAIN_RS.replace("starter::", &format!("{}::", spec.rust_name)),
    )?;
    write_text(
        dest.join(format!("views/{}.lx", spec.crate_name)),
        &VIEW_LX.replace("Starter", &spec.title),
    )?;
    write_text(dest.join("libra.toml"), &rewrite_libra_toml(spec))?;
    write_text(dest.join("tests/unit.rs"), &rewrite_unit(spec))?;
    write_text(dest.join("tests/components.rs"), &rewrite_components(spec))?;
    write_text(dest.join("tests/snapshots.rs"), &rewrite_snapshots(spec))?;
    write_bytes(dest.join("assets/icon.png"), ICON_PNG)?;
    write_text(dest.join("README.md"), &render_readme(spec))?;
    Ok(())
}

fn render_cargo_toml(spec: &Spec) -> String {
    let mut t = CARGO_TOML.to_string();
    t = t.replace(
        "name = \"starter\"",
        &format!("name = \"{}\"", spec.crate_name),
    );
    match &spec.source {
        Source::Path(checkout) => {
            let rel = relative_from(&spec.dest, checkout);
            let rel = rel.to_string_lossy().replace('\\', "/");
            t = t.replace("path = \"../../crates/", &format!("path = \"{rel}/crates/"));
        }
        Source::Git(url) => {
            t = t.replace(
                "libra-ui = { path = \"../../crates/libra\" }",
                &format!("libra-ui = {{ git = \"{url}\" }}"),
            );
            t = t.replace(
                "libra-test = { path = \"../../crates/libra-test\" }",
                &format!("libra-test = {{ git = \"{url}\" }}"),
            );
            t = t.replace(
                "libra-build = { path = \"../../crates/libra-build\" }",
                &format!("libra-build = {{ git = \"{url}\" }}"),
            );
        }
        Source::Registry(version) => {
            t = t.replace(
                "libra-ui = { path = \"../../crates/libra\" }",
                &format!("libra-ui = \"{version}\""),
            );
            t = t.replace(
                "libra-test = { path = \"../../crates/libra-test\" }",
                &format!("libra-test = \"{version}\""),
            );
            t = t.replace(
                "libra-build = { path = \"../../crates/libra-build\" }",
                &format!("libra-build = \"{version}\""),
            );
        }
    }
    t
}

fn rewrite_build_rs(spec: &Spec) -> String {
    BUILD_RS
        .replace("views/starter.lx", &format!("views/{}.lx", spec.crate_name))
        .replace("crate::Starter", &format!("crate::{}", spec.type_name))
}

fn rewrite_lib_rs(spec: &Spec) -> String {
    LIB_RS
        .replace(
            "pub struct Starter",
            &format!("pub struct {}", spec.type_name),
        )
        .replace("impl Starter", &format!("impl {}", spec.type_name))
        .replace("state: Starter", &format!("state: {}", spec.type_name))
        .replace(
            "view: \"../views/starter.lx\"",
            &format!("view: \"../views/{}.lx\"", spec.crate_name),
        )
        .replace(
            "title: \"Libra Starter\"",
            &format!("title: \"{}\"", spec.title),
        )
}

fn rewrite_libra_toml(spec: &Spec) -> String {
    LIBRA_TOML
        .replace("name = \"Starter\"", &format!("name = \"{}\"", spec.title))
        .replace(
            "bundle_id = \"dev.libra.starter\"",
            &format!("bundle_id = \"dev.libra.{}\"", spec.crate_name),
        )
        .replace(
            "app_user_model_id = \"Libra.Starter\"",
            &format!("app_user_model_id = \"Libra.{}\"", spec.type_name),
        )
}

fn rewrite_unit(spec: &Spec) -> String {
    UNIT_RS
        .replace("use starter::", &format!("use {}::", spec.rust_name))
        .replace("Starter", &spec.type_name)
}

fn rewrite_components(spec: &Spec) -> String {
    COMPONENTS_RS
        .replace("use starter::", &format!("use {}::", spec.rust_name))
        .replace("Starter", &spec.type_name)
}

fn rewrite_snapshots(spec: &Spec) -> String {
    let snap = format!("{}_initial", spec.crate_name.replace('-', "_"));
    SNAPSHOTS_RS
        .replace("use starter::", &format!("use {}::", spec.rust_name))
        .replace("Starter", &spec.type_name)
        .replace("starter_initial", &snap)
}

fn render_readme(spec: &Spec) -> String {
    format!(
        "# {}\n\nGenerated by `libra new` (PLAN-SDK S3).\n\n```sh\nlibra dev\nlibra test\nlibra build\n```\n",
        spec.title
    )
}

fn relative_from(from_dir: &Path, to: &Path) -> PathBuf {
    let from = from_dir
        .canonicalize()
        .unwrap_or_else(|_| from_dir.to_path_buf());
    let to = to.canonicalize().unwrap_or_else(|_| to.to_path_buf());
    let from_c: Vec<_> = from.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let common = from_c.iter().zip(&to_c).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in 0..from_c.len().saturating_sub(common) {
        out.push("..");
    }
    for c in &to_c[common..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

fn mkdir(path: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_text(path: PathBuf, contents: &str) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_bytes(path: PathBuf, contents: &[u8]) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn crate_name_sanitizes() {
        assert_eq!(crate_name("My App").unwrap(), "my-app");
        assert_eq!(crate_name("notes").unwrap(), "notes");
        assert!(crate_name("123bad").is_err());
    }

    #[test]
    fn pascal_and_title() {
        assert_eq!(pascal_case("my-app"), "MyApp");
        assert_eq!(title_case("my-app"), "My App");
    }

    #[test]
    fn writes_a_path_dep_app() {
        let dir = tempfile::TempDir::new().unwrap();
        let dest = dir.path().join("notes");
        let spec = parse_new_args(&[
            dest.to_str().unwrap().into(),
            "--path".into(),
            repo_root().to_str().unwrap().into(),
        ])
        .unwrap();
        write_app(&spec).unwrap();

        let cargo = std::fs::read_to_string(dest.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"notes\""));
        assert!(!cargo.lines().any(|l| l.trim() == "[workspace]"));
        assert!(cargo.contains("/crates/libra\""));
        assert!(dest.join("views/notes.lx").is_file());
        assert!(dest.join("src/lib.rs").is_file());
        assert!(
            !dest.join("tests/snapshots/notes_initial.png").is_file(),
            "first libra test records the snapshot"
        );
        let lib = std::fs::read_to_string(dest.join("src/lib.rs")).unwrap();
        assert!(lib.contains("pub struct Notes"));
        assert!(lib.contains("view: \"../views/notes.lx\""));
        assert!(crate::app::find_workspace_root(&dest).is_ok());
        let app = crate::app::resolve(dest, None).unwrap();
        assert_eq!(app.crate_name, "notes");
    }

    #[test]
    fn git_rewrite_does_not_keep_path_deps() {
        let spec = Spec {
            dest: PathBuf::from("/tmp/demo"),
            crate_name: "demo".into(),
            rust_name: "demo".into(),
            type_name: "Demo".into(),
            title: "Demo".into(),
            source: Source::Git(DEFAULT_GIT.into()),
        };
        let cargo = render_cargo_toml(&spec);
        assert!(cargo.contains(&format!("git = \"{DEFAULT_GIT}\"")));
        assert!(!cargo.contains("path = \"../../crates/"));
    }

    #[test]
    fn registry_rewrite_uses_libra_ui_versions() {
        let spec = Spec {
            dest: PathBuf::from("/tmp/demo"),
            crate_name: "demo".into(),
            rust_name: "demo".into(),
            type_name: "Demo".into(),
            title: "Demo".into(),
            source: Source::Registry("0.1.0".into()),
        };
        let cargo = render_cargo_toml(&spec);
        assert!(cargo.contains("libra-ui = \"0.1.0\""));
        assert!(cargo.contains("libra-test = \"0.1.0\""));
        assert!(cargo.contains("libra-build = \"0.1.0\""));
        assert!(!cargo.contains("path = \""));
    }

    #[test]
    fn embedded_template_matches_templates_app() {
        let embedded = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("template");
        let canonical = repo_root().join("templates/app");
        // The embedded manifest is not named Cargo.toml — a nested
        // manifest makes `cargo package` drop the directory.
        for (canon, embed) in [
            ("Cargo.toml", "Cargo.toml.tpl"),
            ("build.rs", "build.rs"),
            ("src/lib.rs", "src/lib.rs"),
            ("src/main.rs", "src/main.rs"),
            ("views/starter.lx", "views/starter.lx"),
            ("libra.toml", "libra.toml"),
            ("tests/unit.rs", "tests/unit.rs"),
            ("tests/components.rs", "tests/components.rs"),
            ("tests/snapshots.rs", "tests/snapshots.rs"),
            ("assets/icon.png", "assets/icon.png"),
        ] {
            let a = std::fs::read(canonical.join(canon)).unwrap();
            let b = std::fs::read(embedded.join(embed)).unwrap();
            assert_eq!(a, b, "{canon} drifted from templates/app");
        }
    }
}
