//! Single-instance guard (PLAN-RESIDENCY R4).
//!
//! The first process binds a localhost port and writes that port into
//! the app-data lock file. A second process connects, asks the first to
//! raise its window, and exits. A dead lock (nothing listening) is
//! replaced. Opt-in: callers that do not set `single_instance` never
//! enter here.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

pub enum Role {
    Primary,
    Secondary,
}

pub fn claim(app_id: Option<&str>, on_raise: impl Fn() + Send + 'static) -> std::io::Result<Role> {
    let Some(app_id) = app_id.filter(|id| !id.is_empty()) else {
        eprintln!("[libra] single instance needs Options::app_id; skipping the lock");
        return Ok(Role::Primary);
    };
    let Some(dirs) = directories::ProjectDirs::from("", "", app_id) else {
        return Ok(Role::Primary);
    };
    let dir = dirs.data_dir();
    std::fs::create_dir_all(dir)?;
    claim_in(dir, on_raise)
}

pub fn claim_in(dir: &Path, on_raise: impl Fn() + Send + 'static) -> std::io::Result<Role> {
    let lock = dir.join("instance.lock");
    if let Some(port) = read_port(&lock) {
        if signal(port) {
            return Ok(Role::Secondary);
        }
        let _ = std::fs::remove_file(&lock);
    }
    match bind_primary(&lock, on_raise) {
        Ok(()) => Ok(Role::Primary),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if let Some(port) = read_port(&lock) {
                if signal(port) {
                    return Ok(Role::Secondary);
                }
            }
            Err(e)
        }
        Err(e) => Err(e),
    }
}

fn bind_primary(lock: &Path, on_raise: impl Fn() + Send + 'static) -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock)?;
    writeln!(file, "{port}")?;
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut conn) = conn else { continue };
            let mut buf = [0u8; 8];
            let _ = conn.read(&mut buf);
            on_raise();
        }
    });
    Ok(())
}

fn signal(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else {
        return false;
    };
    stream.write_all(b"raise\n").is_ok()
}

fn read_port(lock: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(lock).ok()?;
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn a_second_claim_signals_the_first_and_does_not_become_primary() {
        let dir = std::env::temp_dir().join(format!(
            "libra-instance-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let hits = Arc::new(Mutex::new(0u32));
        let hits_bg = Arc::clone(&hits);
        let primary = claim_in(&dir, move || {
            *hits_bg.lock().unwrap() += 1;
        })
        .unwrap();
        assert!(matches!(primary, Role::Primary));
        let secondary = claim_in(&dir, || {}).unwrap();
        assert!(matches!(secondary, Role::Secondary));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline && *hits.lock().unwrap() == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*hits.lock().unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
