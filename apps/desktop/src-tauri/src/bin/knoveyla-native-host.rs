//! Chrome Native Messaging bridge.
//!
//! Chrome frames each JSON message with a four-byte native-endian length. This small
//! process contains no data policy or database access: it forwards the authenticated
//! envelope to the already-running Knoveyla core, which remains the sole SQLite writer.

use std::{
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
};

fn main() {
    loop {
        let mut length = [0u8; 4];
        if std::io::stdin().read_exact(&mut length).is_err() {
            break;
        }
        let size = u32::from_le_bytes(length) as usize;
        if size == 0 || size > 256 * 1024 {
            write_response(serde_json::json!({
                "protocolVersion":1,"requestId":"","ok":false,
                "errorCode":"protocol","message":"Invalid Native Messaging frame."
            }));
            continue;
        }
        let mut payload = vec![0; size];
        if std::io::stdin().read_exact(&mut payload).is_err() {
            break;
        }
        let request: serde_json::Value = match serde_json::from_slice(&payload) {
            Ok(value) => value,
            Err(_) => {
                write_response(serde_json::json!({
                    "protocolVersion":1,"requestId":"","ok":false,
                    "errorCode":"protocol","message":"Invalid request."
                }));
                continue;
            }
        };
        let request_id = request
            .get("requestId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let response = native_socket_path()
            .and_then(|path| UnixStream::connect(path).ok())
            .and_then(|mut stream| {
                stream.write_all(&payload).ok()?;
                stream.shutdown(Shutdown::Write).ok()?;
                let mut response = Vec::new();
                stream.read_to_end(&mut response).ok()?;
                serde_json::from_slice::<serde_json::Value>(&response).ok()
            })
            .unwrap_or_else(|| {
                serde_json::json!({
                    "protocolVersion":1,"requestId":request_id,"ok":false,
                    "errorCode":"unavailable","message":"The Knoveyla app is not running."
                })
            });
        write_response(response);
    }
}

fn native_socket_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|directory| {
        directory
            .join("com.knoveyla.desktop")
            .join("native-messaging.sock")
    })
}

fn write_response(value: serde_json::Value) {
    let payload = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    let mut output = std::io::stdout().lock();
    let _ = output.write_all(&(payload.len() as u32).to_le_bytes());
    let _ = output.write_all(&payload);
    let _ = output.flush();
}
