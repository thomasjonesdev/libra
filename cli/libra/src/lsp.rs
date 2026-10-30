//! `libra lsp` (§18.6). Stdio language server. Diagnostics come from
//! the markup parser and the interpreter (`diagnose`). Completion,
//! hover, and go-to-definition stay open — they need the app's
//! reflected State and Action, which this process does not load.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use serde_json::{json, Value};

pub fn lsp() -> ExitCode {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout();
    loop {
        let Some(body) = read_message(&mut reader) else {
            break;
        };
        let msg: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if msg.get("method").and_then(|m| m.as_str()) == Some("exit") {
            break;
        }
        for out in respond(&msg) {
            let _ = write_message(&mut stdout, &out.to_string());
        }
    }
    ExitCode::SUCCESS
}

/// Answer one JSON-RPC message. Notifications produce zero or one
/// outbound notification. Requests produce one response.
pub fn respond(msg: &Value) -> Vec<Value> {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    match method {
        "initialize" => vec![result(
            id,
            json!({
                "capabilities": { "textDocumentSync": 1 },
                "serverInfo": { "name": "libra", "version": env!("CARGO_PKG_VERSION") }
            }),
        )],
        "initialized" => Vec::new(),
        "shutdown" => vec![result(id, Value::Null)],
        "textDocument/didOpen" | "textDocument/didChange" => {
            let Some(text) = document_text(msg, method) else {
                return Vec::new();
            };
            let uri = msg
                .pointer("/params/textDocument/uri")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            vec![publish(&uri, &text)]
        }
        _ => {
            if let Some(id) = id {
                vec![json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("method '{method}' is not implemented") }
                })]
            } else {
                Vec::new()
            }
        }
    }
}

fn document_text(msg: &Value, method: &str) -> Option<String> {
    if method == "textDocument/didOpen" {
        return msg
            .pointer("/params/textDocument/text")
            .and_then(|t| t.as_str())
            .map(str::to_string);
    }
    msg.pointer("/params/contentChanges/0/text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

fn publish(uri: &str, src: &str) -> Value {
    let diagnostics: Vec<Value> = libra_interp::diagnose(src)
        .into_iter()
        .map(|d| {
            let line = d.line.saturating_sub(1);
            let col = d.col.saturating_sub(1);
            json!({
                "range": {
                    "start": { "line": line, "character": col },
                    "end": { "line": line, "character": col }
                },
                "severity": 1,
                "source": "libra",
                "message": d.message
            })
        })
        .collect();
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diagnostics }
    })
}

fn result(id: Option<Value>, value: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": value })
}

pub fn write_message(out: &mut impl Write, body: &str) -> std::io::Result<()> {
    write!(out, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    out.flush()
}

fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(rest) = line
            .trim()
            .strip_prefix("Content-Length:")
        {
            len = rest.trim().parse().ok();
        }
    }
    let len = len?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_advertises_full_sync() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
        let out = respond(&msg);
        assert_eq!(out[0]["result"]["capabilities"]["textDocumentSync"], 1);
    }

    #[test]
    fn an_unknown_element_is_a_diagnostic() {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///view.lx",
                    "text": "<Column><Nope /></Column>"
                }
            }
        });
        let out = respond(&msg);
        let diags = &out[0]["params"]["diagnostics"];
        assert_eq!(out[0]["method"], "textDocument/publishDiagnostics");
        assert!(diags[0]["message"].as_str().unwrap().contains("unknown"));
        assert_eq!(diags[0]["range"]["start"]["line"], 0);
    }

    #[test]
    fn a_clean_view_publishes_no_diagnostics() {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///view.lx" },
                "contentChanges": [{ "text": "<Column><Text>Hi</Text></Column>" }]
            }
        });
        let out = respond(&msg);
        assert!(out[0]["params"]["diagnostics"].as_array().unwrap().is_empty());
    }
}
