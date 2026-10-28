//! Read-only 4C Insight host monitor. No cohort files or filesystem browsing.
//! Binds IPv4 loopback only. Each launch uses an ephemeral authorization token.
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{System, MINIMUM_CPU_UPDATE_INTERVAL};
use uuid::Uuid;

const PORT: u16 = 47831;
const DEFAULT_ORIGIN: &str = "https://singjc.github.io";
const MAX_HEADER_BYTES: usize = 8192;

fn allowed_origin(value: &str) -> bool {
    if value == DEFAULT_ORIGIN {
        return true;
    }
    // Local development origins are accepted only by explicit CLI opt-in.
    // The caller always supplies a full origin, never a path or wildcard.
    let port = value
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| value.strip_prefix("http://localhost:"));
    port.and_then(|text| text.parse::<u16>().ok())
        .is_some_and(|number| number > 0)
}

fn parse_args() -> Result<String, String> {
    let mut origin = DEFAULT_ORIGIN.to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-origin" => {
                origin = args.next().ok_or("Missing --allow-origin value")?;
                if !allowed_origin(&origin) {
                    return Err("Unsupported origin; use the official Pages URL or explicit local development origin".into());
                }
            }
            "--help" | "-h" => {
                println!("4C Insight host monitor\nUsage: four-c-system-monitor [--allow-origin http://127.0.0.1:8000]\nDefault allowed origin: {DEFAULT_ORIGIN}\nRead-only endpoint: http://127.0.0.1:{PORT}/v1/metrics");
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown option: {arg}")),
        }
    }
    Ok(origin)
}

fn read_request(
    stream: &mut TcpStream,
) -> std::io::Result<Option<(String, String, HashMap<String, String>)>> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut bytes = Vec::with_capacity(1024);
    let mut buf = [0u8; 1024];
    loop {
        let count = stream.read(&mut buf)?;
        if count == 0 {
            return Ok(None);
        }
        bytes.extend_from_slice(&buf[..count]);
        if bytes.len() > MAX_HEADER_BYTES {
            return Ok(None);
        }
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let header = match std::str::from_utf8(&bytes) {
        Ok(header) => header,
        Err(_) => return Ok(None),
    };
    let mut lines = header.split("\r\n");
    let mut request = match lines.next() {
        Some(line) => line.split_ascii_whitespace(),
        None => return Ok(None),
    };
    let method = match request.next() {
        Some(s) => s.to_owned(),
        None => return Ok(None),
    };
    let path = match request.next() {
        Some(s) => s.to_owned(),
        None => return Ok(None),
    };
    if request.next() != Some("HTTP/1.1") || request.next().is_some() {
        return Ok(None);
    }
    let mut fields = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (key, value) = match line.split_once(':') {
            Some(pair) => pair,
            None => return Ok(None),
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if fields.contains_key(&key) {
            return Ok(None);
        }
        fields.insert(key, value.to_owned());
    }
    Ok(Some((method, path, fields)))
}

fn send_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
    origin: Option<&str>,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nVary: Origin\r\n", body.len());
    if let Some(value) = origin {
        head.push_str(&format!("Access-Control-Allow-Origin: {value}\r\nAccess-Control-Allow-Headers: Authorization\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Max-Age: 600\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())
}

fn handle_connection(
    mut stream: TcpStream,
    origin: &str,
    token: &str,
    system: &mut System,
) -> std::io::Result<()> {
    let (method, path, headers) = match read_request(&mut stream)? {
        Some(request) => request,
        None => return Ok(()),
    };
    let host_ok = headers
        .get("host")
        .is_some_and(|v| v == &format!("127.0.0.1:{PORT}"));
    let request_origin_ok = headers.get("origin").is_some_and(|v| v == origin);
    if !host_ok || !request_origin_ok {
        return send_response(
            &mut stream,
            "403 Forbidden",
            "{\"error\":\"origin denied\"}",
            None,
        );
    }
    if path != "/v1/metrics" {
        return send_response(
            &mut stream,
            "404 Not Found",
            "{\"error\":\"not found\"}",
            Some(origin),
        );
    }
    if method == "OPTIONS" {
        let okay = headers
            .get("access-control-request-method")
            .is_some_and(|v| v == "GET")
            && headers
                .get("access-control-request-headers")
                .is_some_and(|v| {
                    v.split(',')
                        .all(|part| part.trim().eq_ignore_ascii_case("authorization"))
                });
        return if okay {
            send_response(&mut stream, "204 No Content", "", Some(origin))
        } else {
            send_response(
                &mut stream,
                "403 Forbidden",
                "{\"error\":\"preflight denied\"}",
                Some(origin),
            )
        };
    }
    if method != "GET" {
        return send_response(
            &mut stream,
            "405 Method Not Allowed",
            "{\"error\":\"read only\"}",
            Some(origin),
        );
    }
    let expected_auth = format!("Bearer {token}");
    if headers
        .get("authorization")
        .is_none_or(|v| v != &expected_auth)
    {
        return send_response(
            &mut stream,
            "401 Unauthorized",
            "{\"error\":\"invalid access token\"}",
            Some(origin),
        );
    }
    system.refresh_memory();
    system.refresh_cpu_usage();
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let snapshot = json!({
        "schema": 1,
        "source": "4c-native-sysinfo",
        "timestamp_ms": timestamp_ms,
        "total_memory_bytes": system.total_memory(),
        "available_memory_bytes": system.available_memory(),
        "used_memory_bytes": system.used_memory(),
        "total_swap_bytes": system.total_swap(),
        "used_swap_bytes": system.used_swap(),
        "logical_cpus": system.cpus().len(),
        "cpu_usage_percent": system.global_cpu_usage(),
    });
    send_response(&mut stream, "200 OK", &snapshot.to_string(), Some(origin))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let origin =
        parse_args().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let listener = TcpListener::bind(("127.0.0.1", PORT))?;
    let token = Uuid::new_v4().simple().to_string();
    let mut system = System::new_all();
    system.refresh_memory();
    system.refresh_cpu_usage();
    std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();

    println!("4C Insight optional system monitor");
    println!("Local address: http://127.0.0.1:{PORT}/v1/metrics");
    println!("Allowed browser origin: {origin}");
    println!("Session access token (paste into 4C Insight > Settings):\n{token}");
    println!("Read-only system metrics. No cohort files are accessed.");
    println!("Keep this terminal open while monitoring. Ctrl+C stops the service.");
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(err) = handle_connection(stream, &origin, &token, &mut system) {
                    eprintln!("Request error: {err}");
                }
            }
            Err(err) => eprintln!("Connection error: {err}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::allowed_origin;
    #[test]
    fn origin_validation_is_strict() {
        assert!(allowed_origin("https://singjc.github.io"));
        assert!(allowed_origin("http://127.0.0.1:8000"));
        assert!(!allowed_origin("https://other.example"));
        assert!(!allowed_origin("http://127.0.0.1:8000/extra"));
        assert!(!allowed_origin("http://localhost:8000.evil.test"));
    }
}
