//! `libra icon` (ARCHITECTURE.md §20, PLAN-RELEASE) — one square PNG
//! becomes the desktop icon set. No phone sizes.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const ICNS_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 256];
const HICOLOR_SIZES: &[u32] = &[32, 64, 128, 256, 512];

pub fn icon(args: &[String]) -> ExitCode {
    let mut source: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-o" | "--out" => {
                let Some(dir) = iter.next() else {
                    eprintln!("libra icon: -o needs a directory");
                    return ExitCode::from(2);
                };
                out = Some(PathBuf::from(dir));
            }
            "-h" | "--help" => {
                usage();
                return ExitCode::SUCCESS;
            }
            s if s.starts_with('-') => {
                eprintln!("libra icon: unknown flag {s}");
                usage();
                return ExitCode::from(2);
            }
            s => {
                if source.is_some() {
                    eprintln!("libra icon: unexpected extra argument {s}");
                    return ExitCode::from(2);
                }
                source = Some(PathBuf::from(s));
            }
        }
    }
    let Some(source) = source else {
        usage();
        return ExitCode::from(2);
    };
    let out = out.unwrap_or_else(|| default_out_dir(&source));
    match generate(&source, &out) {
        Ok(written) => {
            eprintln!(
                "libra icon: wrote {} files into {}",
                written.len(),
                out.display()
            );
            for path in &written {
                eprintln!("  {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("libra icon: {e}");
            ExitCode::from(1)
        }
    }
}

fn usage() {
    eprintln!("usage: libra icon <png> [-o dir]");
    eprintln!("  one square PNG, at least 512 px (1024 preferred)");
    eprintln!("  writes png, icns, ico, and hicolor sizes for desktop bundles");
}

fn default_out_dir(source: &Path) -> PathBuf {
    source
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("icons")
}

/// Resize `source` into `out_dir`. Returns the files written.
pub fn generate(source: &Path, out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let img = image::open(source)
        .map_err(|e| format!("read {}: {e}", source.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    if w != h {
        return Err(format!(
            "{} is {w}×{h}; the source must be square",
            source.display()
        ));
    }
    if w < 512 {
        return Err(format!(
            "{} is {w} px; use at least 512 (1024 preferred)",
            source.display()
        ));
    }

    std::fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let mut written = Vec::new();

    let master_size = w.min(1024);
    let master = resize(&img, master_size);
    let master_path = out_dir.join("icon.png");
    master
        .save(&master_path)
        .map_err(|e| format!("write {}: {e}", master_path.display()))?;
    written.push(master_path);

    for size in [32, 128] {
        let path = out_dir.join(format!("{size}x{size}.png"));
        resize(&img, size)
            .save(&path)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(path);
    }
    let retina = out_dir.join("128x128@2x.png");
    resize(&img, 256)
        .save(&retina)
        .map_err(|e| format!("write {}: {e}", retina.display()))?;
    written.push(retina);

    for size in HICOLOR_SIZES {
        let dir = out_dir.join(format!("hicolor/{size}x{size}/apps"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let path = dir.join("icon.png");
        resize(&img, *size)
            .save(&path)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(path);
    }

    let icns_path = out_dir.join("icon.icns");
    write_icns(&img, &icns_path)?;
    written.push(icns_path);

    let ico_path = out_dir.join("icon.ico");
    write_ico(&img, &ico_path)?;
    written.push(ico_path);

    Ok(written)
}

fn resize(img: &image::RgbaImage, size: u32) -> image::RgbaImage {
    if img.width() == size && img.height() == size {
        return img.clone();
    }
    image::imageops::resize(img, size, size, image::imageops::FilterType::Lanczos3)
}

fn write_icns(img: &image::RgbaImage, path: &Path) -> Result<(), String> {
    let mut family = icns::IconFamily::new();
    for size in ICNS_SIZES {
        let rgba = resize(img, *size).into_raw();
        let icon = icns::Image::from_data(icns::PixelFormat::RGBA, *size, *size, rgba)
            .map_err(|e| format!("icns {size}: {e}"))?;
        family
            .add_icon(&icon)
            .map_err(|e| format!("icns {size}: {e}"))?;
    }
    let file =
        std::fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    family
        .write(file)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn write_ico(img: &image::RgbaImage, path: &Path) -> Result<(), String> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let rgba = resize(img, *size).into_raw();
        let image = ico::IconImage::from_rgba_data(*size, *size, rgba);
        let entry = ico::IconDirEntry::encode(&image).map_err(|e| format!("ico {size}: {e}"))?;
        dir.add_entry(entry);
    }
    let file =
        std::fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    dir.write(file)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_png_writes_desktop_icons() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.png");
        let img = image::RgbaImage::from_pixel(512, 512, image::Rgba([20, 40, 60, 255]));
        img.save(&source).unwrap();
        let out = dir.path().join("icons");
        let written = generate(&source, &out).unwrap();
        assert!(written.iter().any(|p| p.ends_with("icon.png")));
        assert!(written.iter().any(|p| p.ends_with("icon.icns")));
        assert!(written.iter().any(|p| p.ends_with("icon.ico")));
        assert!(written.iter().any(|p| p.ends_with("32x32.png")));
        assert!(out.join("hicolor/128x128/apps/icon.png").is_file());

        let png = image::open(out.join("icon.png")).unwrap().to_rgba8();
        assert_eq!(png.dimensions(), (512, 512));
        let small = image::open(out.join("32x32.png")).unwrap().to_rgba8();
        assert_eq!(small.dimensions(), (32, 32));

        let icns = std::fs::read(out.join("icon.icns")).unwrap();
        assert_eq!(&icns[..4], b"icns");
        let ico_bytes = std::fs::read(out.join("icon.ico")).unwrap();
        assert_eq!(&ico_bytes[..4], &[0, 0, 1, 0]);
    }

    #[test]
    fn rejects_a_non_square_or_small_png() {
        let dir = tempfile::tempdir().unwrap();
        let wide = dir.path().join("wide.png");
        image::RgbaImage::from_pixel(600, 400, image::Rgba([0, 0, 0, 255]))
            .save(&wide)
            .unwrap();
        assert!(generate(&wide, &dir.path().join("out")).is_err());

        let small = dir.path().join("small.png");
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]))
            .save(&small)
            .unwrap();
        assert!(generate(&small, &dir.path().join("out")).is_err());
    }
}
