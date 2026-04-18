use crate::DebugDataError;
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpResponse {
    pub(crate) status_code: u16,
    pub(crate) content_type: &'static str,
    pub(crate) body: Vec<u8>,
}

pub(crate) fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, DebugDataError> {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut buffer = Vec::with_capacity(32 * 1024);
    let mut chunk = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        let bytes_read = match stream.read(&mut chunk) {
            Ok(bytes_read) => bytes_read,
            Err(source)
                if matches!(
                    source.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if !buffer.is_empty() {
                    break;
                }
                continue;
            }
            Err(source) => {
                return Err(DebugDataError::Io {
                    path: "tcp-stream-read".into(),
                    source,
                })
            }
        };
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if header_end.is_none() {
            header_end = find_header_end(&buffer);
            if let Some(end) = header_end {
                let head = String::from_utf8_lossy(&buffer[..end]);
                content_length = parse_content_length(&head);
                let body_bytes = buffer.len().saturating_sub(end + 4);
                if has_expect_continue(&head) && body_bytes < content_length {
                    write_continue_response(stream)?;
                }
            }
        }

        if let Some(end) = header_end {
            let body_bytes = buffer.len().saturating_sub(end + 4);
            if body_bytes >= content_length {
                break;
            }
        }
    }

    let request = String::from_utf8_lossy(&buffer);
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut parts = head
        .lines()
        .next()
        .unwrap_or("GET / HTTP/1.1")
        .split_whitespace();

    let body_bytes = if content_length == 0 {
        body.as_bytes().to_vec()
    } else {
        body.as_bytes()
            .iter()
            .take(content_length)
            .copied()
            .collect()
    };

    Ok(HttpRequest {
        method: parts.next().unwrap_or("GET").to_string(),
        path: parts.next().unwrap_or("/").to_string(),
        body: body_bytes,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn has_expect_continue(head: &str) -> bool {
    head.lines().any(|line| {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("expect")
            && value.trim().eq_ignore_ascii_case("100-continue")
    })
}

fn write_continue_response(stream: &mut TcpStream) -> Result<(), DebugDataError> {
    stream
        .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-write".into(),
            source,
        })
}

pub(crate) fn write_http_response(
    stream: &mut TcpStream,
    response: &HttpResponse,
) -> Result<(), DebugDataError> {
    let status_text = match response.status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status_code,
        status_text,
        response.content_type,
        response.body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(&response.body))
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-write".into(),
            source,
        })
}

pub(crate) fn json_response(status_code: u16, value: &impl Serialize) -> HttpResponse {
    let body = serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{}".to_vec());
    HttpResponse {
        status_code,
        content_type: "application/json; charset=utf-8",
        body,
    }
}

pub(crate) fn html_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "text/html; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

pub(crate) fn javascript_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "application/javascript; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

pub(crate) fn css_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "text/css; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

pub(crate) fn file_response(path: &Path, content_type: &'static str) -> HttpResponse {
    match fs::read(path) {
        Ok(body) => HttpResponse {
            status_code: 200,
            content_type,
            body,
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => not_found_response(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact"),
        ),
        Err(err) => internal_error_response(&format!("failed to read {}: {err}", path.display())),
    }
}

pub(crate) fn bad_request_response(message: &str) -> HttpResponse {
    HttpResponse {
        status_code: 400,
        content_type: "text/plain; charset=utf-8",
        body: format!("bad request: {message}\n").into_bytes(),
    }
}

pub(crate) fn not_found_response(path: &str) -> HttpResponse {
    HttpResponse {
        status_code: 404,
        content_type: "text/plain; charset=utf-8",
        body: format!("not found: {path}\n").into_bytes(),
    }
}

pub(crate) fn internal_error_response(message: &str) -> HttpResponse {
    HttpResponse {
        status_code: 500,
        content_type: "text/plain; charset=utf-8",
        body: format!("internal error: {message}\n").into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::{find_header_end, has_expect_continue, parse_content_length};

    #[test]
    fn parse_content_length_reads_case_insensitive_header() {
        let head = "POST /api/chat/send HTTP/1.1\r\nHost: localhost\r\nContent-Length: 27\r\n\r\n";
        assert_eq!(parse_content_length(head), 27);
    }

    #[test]
    fn detect_expect_continue_header() {
        let head =
            "POST /api/chat/send HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: 27\r\n\r\n";
        assert!(has_expect_continue(head));
    }

    #[test]
    fn find_header_end_locates_separator() {
        let raw = b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\nbody";
        assert_eq!(find_header_end(raw), Some(32));
    }
}
