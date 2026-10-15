use std::io::{BufRead, Write};

/// Read one LSP frame from `reader`.
///
/// Returns `Ok(None)` on clean EOF before any header byte.
/// Returns `Ok(Some(body))` per framed message.
pub fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut content_len: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let parsed = value.trim().parse::<usize>().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "bad content length")
            })?;
            content_len = Some(parsed);
        }
    }
    let Some(len) = content_len else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing content length",
        ));
    };
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    let body = String::from_utf8(buf)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "body is not utf8"))?;
    Ok(Some(body))
}

/// Write one LSP frame to `writer`.
pub fn write_message(writer: &mut impl Write, body: &str) -> std::io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    writer.flush()
}

/// Run the read-write loop until EOF.
///
/// Each input frame goes to `on_message`. A `Some(reply)` reply is framed back.
pub fn serve_loop(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    on_message: &mut dyn FnMut(&str) -> Option<String>,
) -> std::io::Result<()> {
    while let Some(body) = read_message(reader)? {
        if let Some(reply) = on_message(&body) {
            write_message(writer, &reply)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn framed(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }

    #[test]
    fn roundtrips_single_message() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let mut input = Cursor::new(framed(body));
        let got = read_message(&mut input).expect("read must succeed");
        assert_eq!(got.as_deref(), Some(body));
    }

    #[test]
    fn reads_two_messages_back_to_back() {
        let first = r#"{"jsonrpc":"2.0","id":1,"method":"a"}"#;
        let second = r#"{"jsonrpc":"2.0","id":2,"method":"b"}"#;
        let mut raw = framed(first);
        raw.extend_from_slice(&framed(second));
        let mut input = Cursor::new(raw);
        assert_eq!(
            read_message(&mut input).expect("first read").as_deref(),
            Some(first)
        );
        assert_eq!(
            read_message(&mut input).expect("second read").as_deref(),
            Some(second)
        );
        assert!(read_message(&mut input).expect("eof read").is_none());
    }

    #[test]
    fn clean_eof_gives_none() {
        let mut input = Cursor::new(Vec::new());
        assert!(read_message(&mut input).expect("eof read").is_none());
    }

    #[test]
    fn missing_length_is_error() {
        let mut input = Cursor::new(b"Content-Type: application/json\r\n\r\n{}".to_vec());
        assert!(read_message(&mut input).is_err());
    }

    #[test]
    fn loop_echoes_frames_both_ways() {
        let first = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let second = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
        let mut raw = framed(first);
        raw.extend_from_slice(&framed(second));
        let mut input = Cursor::new(raw);
        let mut output = Vec::new();
        serve_loop(&mut input, &mut output, &mut |body| Some(body.to_string()))
            .expect("loop must succeed");
        let mut expected = framed(first);
        expected.extend_from_slice(&framed(second));
        assert_eq!(output, expected);
    }
}
