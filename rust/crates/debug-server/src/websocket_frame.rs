use crate::DebugDataError;
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WsFrame {
    pub(crate) opcode: u8,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn read_frame(stream: &mut TcpStream) -> Result<Option<WsFrame>, DebugDataError> {
    let mut header = [0_u8; 2];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(source) => {
            return Err(DebugDataError::Io {
                path: "websocket-frame-header-read".into(),
                source,
            });
        }
    }
    let opcode = header[0] & 0x0f;
    let masked = header[1] & 0x80 != 0;
    let len = read_payload_len(stream, header[1] & 0x7f)?;
    if len > 1024 * 1024 {
        return Err(DebugDataError::Io {
            path: "websocket-frame-too-large".into(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"),
        });
    }
    let mut mask = [0_u8; 4];
    if masked {
        stream
            .read_exact(&mut mask)
            .map_err(|source| DebugDataError::Io {
                path: "websocket-frame-mask-read".into(),
                source,
            })?;
    }
    let mut payload = vec![0_u8; len as usize];
    stream
        .read_exact(&mut payload)
        .map_err(|source| DebugDataError::Io {
            path: "websocket-frame-payload-read".into(),
            source,
        })?;
    if masked {
        for (idx, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[idx % 4];
        }
    }
    Ok(Some(WsFrame { opcode, payload }))
}

fn read_payload_len(stream: &mut TcpStream, marker: u8) -> Result<u64, DebugDataError> {
    if marker < 126 {
        return Ok(u64::from(marker));
    }
    if marker == 126 {
        let mut ext = [0_u8; 2];
        stream
            .read_exact(&mut ext)
            .map_err(|source| DebugDataError::Io {
                path: "websocket-frame-len16-read".into(),
                source,
            })?;
        return Ok(u64::from(u16::from_be_bytes(ext)));
    }
    let mut ext = [0_u8; 8];
    stream
        .read_exact(&mut ext)
        .map_err(|source| DebugDataError::Io {
            path: "websocket-frame-len64-read".into(),
            source,
        })?;
    Ok(u64::from_be_bytes(ext))
}

pub(crate) fn write_text_frame(stream: &mut TcpStream, text: &str) -> Result<(), DebugDataError> {
    write_frame(stream, 0x1, text.as_bytes())
}

pub(crate) fn write_close_frame(stream: &mut TcpStream) -> Result<(), DebugDataError> {
    write_frame(stream, 0x8, &[])
}

fn write_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> Result<(), DebugDataError> {
    let mut frame = vec![0x80 | opcode];
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream
        .write_all(&frame)
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "websocket-frame-write".into(),
            source,
        })
}
