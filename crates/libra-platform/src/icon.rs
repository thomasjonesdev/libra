//! Window icon loading (§20 / PLAN-PACKAGE) — outside the paint contract.

use std::path::Path;

use winit::window::Icon;

/// Load a square PNG for the window / dock title-bar icon.
pub fn load_png(path: &Path) -> Option<Icon> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    let width = info.width;
    let height = info.height;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut buf).ok()?;
    let pixels = (width as usize).checked_mul(height as usize)?;
    let rgba = match buf.len() / pixels {
        4 => buf,
        3 => {
            let mut rgba = Vec::with_capacity(pixels * 4);
            for chunk in buf.chunks_exact(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(0xff);
            }
            rgba
        }
        2 => {
            let mut rgba = Vec::with_capacity(pixels * 4);
            for chunk in buf.chunks_exact(2) {
                let g = chunk[0];
                rgba.extend_from_slice(&[g, g, g, chunk[1]]);
            }
            rgba
        }
        1 => {
            let mut rgba = Vec::with_capacity(pixels * 4);
            for &g in &buf {
                rgba.extend_from_slice(&[g, g, g, 0xff]);
            }
            rgba
        }
        _ => return None,
    };
    Icon::from_rgba(rgba, width, height).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    #[test]
    fn loads_aws_login_icon_when_present() {
        let path = repo_root().join("applications/aws-login/assets/icon.png");
        if !path.is_file() {
            return;
        }
        assert!(load_png(&path).is_some());
    }
}
