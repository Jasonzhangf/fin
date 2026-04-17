use crate::DebugDataError;
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
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
    let mut buffer = vec![0_u8; 32 * 1024];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-read".into(),
            source,
        })?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut parts = head
        .lines()
        .next()
        .unwrap_or("GET / HTTP/1.1")
        .split_whitespace();

    Ok(HttpRequest {
        method: parts.next().unwrap_or("GET").to_string(),
        path: parts.next().unwrap_or("/").to_string(),
        body: body.as_bytes().to_vec(),
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