//! Automation server (§16). A local socket speaks JSON lines so an
//! external harness can snapshot the semantics tree, query it, dispatch
//! a reflected action, read state, and read the action log — without
//! scraping pixels.
//!
//! `libra dev` sets `LIBRA_AUTOMATION=1`. Debug builds then listen on a
//! unix socket mode `0600` and print the path. Release builds ignore the
//! variable (the manifest gate for release is still open). Screenshot,
//! synthetic input, and perf verbs are not this slice.
//!
//! `#[libra(redact)]` fields stay bindable and are omitted from `state`
//! and from `schema.fields`. Their names appear only under `schema.redacted`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::menu::ShellEvent;

/// One semantics node, geometry included, as the `snapshot` / `query` verbs return it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub(crate) struct NodeSnap {
    #[serde(rename = "testId", skip_serializing_if = "Option::is_none")]
    pub test_id: Option<String>,
    pub role: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Action names plus the state fields an agent may read.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub(crate) struct SchemaSnap {
    pub actions: Vec<String>,
    pub fields: Vec<String>,
    pub redacted: Vec<String>,
}

pub(crate) trait Drive {
    fn nodes(&self) -> Vec<NodeSnap>;
    fn dispatch_action(&mut self, action: &str, arg: Option<String>) -> Result<(), String>;
    fn fields(&self) -> Vec<(String, String)>;
    fn schema(&self) -> SchemaSnap;
    fn log(&self) -> Vec<String>;
}

/// Bindable names minus redacted ones, in `names` order.
pub(crate) fn visible_fields(
    names: &[&str],
    redacted: &[&str],
    value: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    names
        .iter()
        .filter(|name| !redacted.contains(name))
        .filter_map(|name| value(name).map(|v| ((*name).to_string(), v)))
        .collect()
}

pub(crate) fn schema_snap(actions: &[&str], fields: &[&str], redacted: &[&str]) -> SchemaSnap {
    SchemaSnap {
        actions: actions.iter().map(|s| (*s).to_string()).collect(),
        fields: fields
            .iter()
            .filter(|name| !redacted.contains(name))
            .map(|s| (*s).to_string())
            .collect(),
        redacted: redacted.iter().map(|s| (*s).to_string()).collect(),
    }
}

#[derive(serde::Deserialize)]
struct Request {
    #[serde(default)]
    id: u64,
    verb: String,
    #[serde(default, rename = "testId")]
    test_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    arg: Option<String>,
}

fn fail(id: u64, error: &str) -> String {
    json!({"id": id, "ok": false, "error": error}).to_string()
}

fn ok(id: u64, extra: Value) -> String {
    let mut body = match extra {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    body.insert("id".into(), json!(id));
    body.insert("ok".into(), json!(true));
    Value::Object(body).to_string()
}

/// Answer one JSON line. Transport failures (a dead window) are the
/// listener's job; this only speaks the verb vocabulary.
pub(crate) fn handle_line(drive: &mut dyn Drive, line: &str) -> String {
    let req: Request = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(e) => return fail(0, &format!("bad request: {e}")),
    };
    match req.verb.as_str() {
        "snapshot" => ok(req.id, json!({"nodes": drive.nodes()})),
        "query" => {
            if req.test_id.is_none() && req.text.is_none() && req.role.is_none() {
                return fail(req.id, "query needs testId, text, or role");
            }
            let nodes: Vec<_> = drive
                .nodes()
                .into_iter()
                .filter(|node| {
                    req.test_id
                        .as_ref()
                        .is_none_or(|id| node.test_id.as_deref() == Some(id))
                        && req.text.as_ref().is_none_or(|text| node.text == *text)
                        && req.role.as_ref().is_none_or(|role| node.role == *role)
                })
                .collect();
            ok(req.id, json!({"nodes": nodes}))
        }
        "dispatch" => {
            let Some(action) = req.action.as_deref() else {
                return fail(req.id, "dispatch needs action");
            };
            match drive.dispatch_action(action, req.arg) {
                Ok(()) => ok(req.id, json!({})),
                Err(e) => fail(req.id, &e),
            }
        }
        "state" => {
            let fields: serde_json::Map<String, Value> = drive
                .fields()
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect();
            ok(req.id, json!({"fields": fields}))
        }
        "schema" => ok(req.id, serde_json::to_value(drive.schema()).unwrap_or(json!({}))),
        "log" => ok(req.id, json!({"entries": drive.log()})),
        other => fail(req.id, &format!("unknown verb '{other}'")),
    }
}

/// Debug + `LIBRA_AUTOMATION=1` → listen and return the socket path.
/// Otherwise `None`. A bind failure is logged and the app still opens.
pub(crate) fn start<M: Send + 'static>(
    proxy: winit::event_loop::EventLoopProxy<ShellEvent<M>>,
) -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    if std::env::var("LIBRA_AUTOMATION").ok().as_deref() != Some("1") {
        return None;
    }
    #[cfg(unix)]
    {
        let path = std::env::temp_dir().join(format!("libra-{}.sock", std::process::id()));
        match listen(path.clone(), move |line| {
            let (tx, rx) = std::sync::mpsc::channel();
            if proxy
                .send_event(ShellEvent::Automate { line, reply: tx })
                .is_err()
            {
                return fail(0, "window closed");
            }
            rx.recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| fail(0, "timed out"))
        }) {
            Ok(()) => {
                eprintln!("[libra] automation {}", path.display());
                Some(path)
            }
            Err(e) => {
                eprintln!("[libra] automation refused: {e}");
                None
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = proxy;
        eprintln!("[libra] automation server needs a unix socket");
        None
    }
}

#[cfg(unix)]
fn listen(
    path: PathBuf,
    answer: impl Fn(String) -> String + Send + Sync + 'static,
) -> std::io::Result<()> {
    let listener = bind_socket(&path)?;
    std::thread::spawn(move || {
        let answer = std::sync::Arc::new(answer);
        for conn in listener.incoming() {
            let Ok(stream) = conn else { continue };
            let answer = std::sync::Arc::clone(&answer);
            std::thread::spawn(move || read_loop(stream, answer.as_ref()));
        }
    });
    Ok(())
}

#[cfg(unix)]
fn bind_socket(path: &Path) -> std::io::Result<std::os::unix::net::UnixListener> {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(unix)]
fn read_loop(stream: std::os::unix::net::UnixStream, answer: impl Fn(String) -> String) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut writer = stream;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let response = answer(line.trim().to_string());
                if writeln!(writer, "{response}").is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::sync::{Arc, Mutex};

    struct Fake {
        count: i64,
    }

    impl Drive for Fake {
        fn nodes(&self) -> Vec<NodeSnap> {
            vec![NodeSnap {
                test_id: Some("save".into()),
                role: "Button".into(),
                text: "Save".into(),
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            }]
        }
        fn dispatch_action(&mut self, action: &str, arg: Option<String>) -> Result<(), String> {
            match action {
                "increment" => {
                    self.count += 1;
                    Ok(())
                }
                "set" => {
                    self.count = arg
                        .ok_or_else(|| "set needs arg".to_string())?
                        .parse()
                        .map_err(|_| "bad arg".to_string())?;
                    Ok(())
                }
                other => Err(format!("unknown action '{other}'")),
            }
        }
        fn fields(&self) -> Vec<(String, String)> {
            visible_fields(&["count", "token"], &["token"], |name| match name {
                "count" => Some(self.count.to_string()),
                "token" => Some("secret".into()),
                _ => None,
            })
        }
        fn schema(&self) -> SchemaSnap {
            schema_snap(&["increment", "set"], &["count", "token"], &["token"])
        }
        fn log(&self) -> Vec<String> {
            vec!["Increment".into()]
        }
    }

    #[test]
    fn redacted_values_stay_out_of_state_and_schema() {
        let mut fake = Fake { count: 3 };
        let state: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":1,"verb":"state"}"#)).unwrap();
        assert_eq!(state["fields"]["count"], "3");
        assert!(state["fields"].get("token").is_none());
        let schema: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":2,"verb":"schema"}"#)).unwrap();
        assert_eq!(schema["fields"], json!(["count"]));
        assert_eq!(schema["redacted"], json!(["token"]));
        assert_eq!(schema["actions"], json!(["increment", "set"]));
    }

    #[test]
    fn query_and_dispatch_round_trip() {
        let mut fake = Fake { count: 0 };
        let snap: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":1,"verb":"snapshot"}"#)).unwrap();
        assert_eq!(snap["nodes"][0]["testId"], "save");
        assert_eq!(snap["nodes"][0]["role"], "Button");
        let queried: Value = serde_json::from_str(&handle_line(
            &mut fake,
            r#"{"id":2,"verb":"query","role":"Button","text":"Save"}"#,
        ))
        .unwrap();
        assert_eq!(queried["nodes"].as_array().unwrap().len(), 1);
        let miss: Value = serde_json::from_str(&handle_line(
            &mut fake,
            r#"{"id":3,"verb":"query","testId":"missing"}"#,
        ))
        .unwrap();
        assert!(miss["nodes"].as_array().unwrap().is_empty());
        let bare: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":4,"verb":"query"}"#)).unwrap();
        assert_eq!(bare["ok"], false);
        handle_line(&mut fake, r#"{"id":5,"verb":"dispatch","action":"increment"}"#);
        handle_line(&mut fake, r#"{"id":6,"verb":"dispatch","action":"set","arg":"9"}"#);
        assert_eq!(fake.count, 9);
        let bad: Value = serde_json::from_str(&handle_line(
            &mut fake,
            r#"{"id":7,"verb":"dispatch","action":"nope"}"#,
        ))
        .unwrap();
        assert_eq!(bad["ok"], false);
        let unknown: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":8,"verb":"perf"}"#)).unwrap();
        assert!(unknown["error"].as_str().unwrap().contains("unknown verb"));
        let log: Value = serde_json::from_str(&handle_line(&mut fake, r#"{"id":9,"verb":"log"}"#)).unwrap();
        assert_eq!(log["entries"], json!(["Increment"]));
    }

    #[test]
    fn the_socket_is_local_and_round_trips() {
        let path = std::env::temp_dir().join(format!(
            "libra-automate-test-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let drive = Arc::new(Mutex::new(Fake { count: 1 }));
        let drive2 = Arc::clone(&drive);
        listen(path.clone(), move |line| {
            let mut drive = drive2.lock().unwrap();
            handle_line(&mut *drive, &line)
        })
        .unwrap();
        struct Unlink(PathBuf);
        impl Drop for Unlink {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _unlink = Unlink(path.clone());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let mut stream = UnixStream::connect(&path).unwrap();
        writeln!(stream, r#"{{"id":1,"verb":"state"}}"#).unwrap();
        stream.flush().unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let body: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["fields"]["count"], "1");
    }
}
