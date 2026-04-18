use crate::DebugDataError;
use std::{
    io::Write,
    net::TcpStream,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime},
};

const WATCH_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) fn stream_runtime_updates(
    stream: &mut TcpStream,
    runtime_home: &Path,
) -> Result<(), DebugDataError> {
    write_headers(stream)?;

    let mut current_revision = runtime_revision(runtime_home);
    write_event(stream, "runtime.ready", &current_revision)?;
    let mut last_heartbeat = Instant::now();

    loop {
        thread::sleep(WATCH_INTERVAL);
        let next_revision = runtime_revision(runtime_home);
        if next_revision != current_revision {
            current_revision = next_revision.clone();
            write_event(stream, "runtime.updated", &next_revision)?;
            last_heartbeat = Instant::now();
            continue;
        }

        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            stream
                .write_all(b": heartbeat\n\n")
                .map_err(|source| DebugDataError::Io {
                    path: "tcp-stream-write".into(),
                    source,
                })?;
            stream.flush().map_err(|source| DebugDataError::Io {
                path: "tcp-stream-write".into(),
                source,
            })?;
            last_heartbeat = Instant::now();
        }
    }
}

fn write_headers(stream: &mut TcpStream) -> Result<(), DebugDataError> {
    let header = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/event-stream; charset=utf-8\r\n",
        "Cache-Control: no-store\r\n",
        "Connection: keep-alive\r\n",
        "X-Accel-Buffering: no\r\n\r\n"
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-write".into(),
            source,
        })
}

fn write_event(
    stream: &mut TcpStream,
    event_name: &str,
    revision: &str,
) -> Result<(), DebugDataError> {
    let body = format!("event: {event_name}\ndata: {{\"revision\":\"{revision}\"}}\n\n");
    stream
        .write_all(body.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-write".into(),
            source,
        })
}

fn runtime_revision(runtime_home: &Path) -> String {
    let snapshot = file_stamp(&runtime_home.join("runtime/projections/current_snapshot.json"));
    let last_run = file_stamp(&runtime_home.join("runtime/current/last_run.json"));
    format!("snapshot={snapshot};last_run={last_run}")
}

fn file_stamp(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|value| value.as_millis())
                .unwrap_or_default();
            format!("{}:{modified}", metadata.len())
        }
        Err(_) => "missing".into(),
    }
}
