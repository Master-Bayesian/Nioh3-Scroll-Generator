//! Bounded length-prefixed frame transport.
//!
//! Byte-for-byte the shipped `nioh3_scroll_editor/worker_transport.py`
//! contract: a four-byte little-endian payload length followed by UTF-8 JSON,
//! a 4 MiB cap, and a clean `None` on a frame boundary EOF. Frame-level faults
//! are fatal exactly as they are in the Python worker, which lets them
//! propagate out of its read loop.

use std::io::{Read, Write};

use serde_json::Value;

/// `MAX_FRAME_BYTES` from the shipped transport and request contracts.
pub const MAX_FRAME_BYTES: u32 = 4 * 1024 * 1024;

/// Frame-level failures. Each one terminates the worker, matching Python.
#[derive(Debug)]
pub enum TransportError {
    /// The stream ended inside a declared frame.
    TruncatedFrame,
    /// Declared length was zero or above [`MAX_FRAME_BYTES`].
    FrameTooLarge,
    /// The frame body was not UTF-8 JSON.
    InvalidJson(String),
    /// The underlying stream failed.
    Io(std::io::Error),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::TruncatedFrame => formatter.write_str("Truncated frame"),
            TransportError::FrameTooLarge => {
                formatter.write_str("Frame size exceeds protocol limit")
            }
            TransportError::InvalidJson(message) => {
                write!(formatter, "Invalid frame JSON: {message}")
            }
            TransportError::Io(error) => write!(formatter, "Frame stream error: {error}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Read one frame, or `Ok(None)` on a clean EOF before any header byte.
pub fn read_frame<R: Read>(stream: &mut R) -> Result<Option<Value>, TransportError> {
    let mut first = [0u8; 1];
    if stream.read(&mut first).map_err(TransportError::Io)? == 0 {
        return Ok(None);
    }
    let mut header = [0u8; 4];
    header[0] = first[0];
    read_exact(stream, &mut header[1..])?;
    let size = u32::from_le_bytes(header);
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge);
    }
    let mut body = vec![0u8; size as usize];
    read_exact(stream, &mut body)?;
    let text =
        String::from_utf8(body).map_err(|error| TransportError::InvalidJson(error.to_string()))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| TransportError::InvalidJson(error.to_string()))
}

/// Write one compact UTF-8 JSON frame.
pub fn write_frame<W: Write>(stream: &mut W, payload: &Value) -> Result<(), TransportError> {
    let raw = serde_json::to_vec(payload)
        .map_err(|error| TransportError::InvalidJson(error.to_string()))?;
    if raw.len() > MAX_FRAME_BYTES as usize {
        return Err(TransportError::FrameTooLarge);
    }
    stream
        .write_all(&(raw.len() as u32).to_le_bytes())
        .map_err(TransportError::Io)?;
    stream.write_all(&raw).map_err(TransportError::Io)?;
    stream.flush().map_err(TransportError::Io)?;
    Ok(())
}

fn read_exact<R: Read>(stream: &mut R, buffer: &mut [u8]) -> Result<(), TransportError> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = stream
            .read(&mut buffer[filled..])
            .map_err(TransportError::Io)?;
        if read == 0 {
            return Err(TransportError::TruncatedFrame);
        }
        filled += read;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frame(body: &[u8]) -> Vec<u8> {
        let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn round_trips_one_frame_and_reports_clean_eof() {
        let bytes = frame(br#"{"protocol":1}"#);
        let mut stream = Cursor::new(bytes);
        let value = read_frame(&mut stream)
            .expect("frame reads")
            .expect("frame present");
        assert_eq!(value["protocol"], 1);
        assert!(read_frame(&mut stream).expect("clean EOF").is_none());
    }

    #[test]
    fn rejects_declared_lengths_outside_the_protocol_limit() {
        for size in [0u32, MAX_FRAME_BYTES + 1] {
            let mut stream = Cursor::new(size.to_le_bytes().to_vec());
            assert!(matches!(
                read_frame(&mut stream),
                Err(TransportError::FrameTooLarge)
            ));
        }
    }

    #[test]
    fn rejects_a_truncated_body() {
        let mut bytes = frame(br#"{"protocol":1}"#);
        bytes.pop();
        let mut stream = Cursor::new(bytes);
        assert!(matches!(
            read_frame(&mut stream),
            Err(TransportError::TruncatedFrame)
        ));
    }

    #[test]
    fn rejects_invalid_json_bodies() {
        let mut stream = Cursor::new(frame(b"not json"));
        assert!(matches!(
            read_frame(&mut stream),
            Err(TransportError::InvalidJson(_))
        ));
    }

    #[test]
    fn writes_a_length_prefixed_compact_payload() {
        let mut output = Vec::new();
        write_frame(&mut output, &serde_json::json!({"a": 1})).expect("frame writes");
        assert_eq!(output[..4], 7u32.to_le_bytes());
        assert_eq!(output[4..], br#"{"a":1}"#[..]);
    }
}
