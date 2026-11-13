//! Bounded line reader: never holds more than `MAX_LINE_BYTES` of a line; the rest is drained.
use std::io::{self, BufRead};

pub const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineMeta {
    /// The physical line was longer than `MAX_LINE_BYTES` and was cut.
    pub truncated: bool,
    /// Bytes consumed from the stream, including the newline.
    pub consumed: u64,
}

/// Read one physical line into `buf` (without `\n`, and without one trailing `\r` unless truncated).
/// Returns `None` at EOF. A final line without a trailing newline counts as a line.
pub fn read_line<R: BufRead>(r: &mut R, buf: &mut Vec<u8>) -> io::Result<Option<LineMeta>> {
    buf.clear();
    let mut total = 0usize;
    let mut consumed = 0u64;
    let mut seen = false;
    loop {
        let avail = match r.fill_buf() {
            Ok(a) => a,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if avail.is_empty() {
            break;
        }
        seen = true;
        let (chunk, used, done) = match memchr::memchr(b'\n', avail) {
            Some(i) => (&avail[..i], i + 1, true),
            None => (avail, avail.len(), false),
        };
        total += chunk.len();
        let room = MAX_LINE_BYTES.saturating_sub(buf.len());
        buf.extend_from_slice(&chunk[..room.min(chunk.len())]);
        consumed += used as u64;
        r.consume(used);
        if done {
            break;
        }
    }
    if !seen {
        return Ok(None);
    }
    let truncated = total > MAX_LINE_BYTES;
    if !truncated && buf.last() == Some(&b'\r') {
        buf.pop();
    }
    Ok(Some(LineMeta {
        truncated,
        consumed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    fn lines(data: &[u8]) -> Vec<(Vec<u8>, bool)> {
        let mut r = BufReader::with_capacity(7, Cursor::new(data.to_vec()));
        let mut buf = Vec::new();
        let mut out = vec![];
        while let Some(m) = read_line(&mut r, &mut buf).unwrap() {
            out.push((buf.clone(), m.truncated));
        }
        out
    }

    #[test]
    fn empty_file_has_no_lines() {
        assert!(lines(b"").is_empty());
    }

    #[test]
    fn counts_each_physical_line_once() {
        assert_eq!(lines(b"a\nb\n").len(), 2);
        assert_eq!(lines(b"a\nb").len(), 2, "final line without newline");
        assert_eq!(lines(b"\n").len(), 1, "a lone newline is one empty line");
        assert_eq!(lines(b"a\n\nb\n").len(), 3);
    }

    #[test]
    fn handles_crlf() {
        let l = lines(b"a\r\nb\r\nc");
        assert_eq!(l[0].0, b"a");
        assert_eq!(l[1].0, b"b");
        assert_eq!(l[2].0, b"c");
    }

    #[test]
    fn caps_long_lines_and_drains_remainder() {
        let mut data = vec![b'x'; MAX_LINE_BYTES * 3 + 5];
        data.extend_from_slice(b"\nnext\n");
        let l = lines(&data);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].0.len(), MAX_LINE_BYTES);
        assert!(l[0].1);
        assert_eq!(l[1].0, b"next");
        assert!(!l[1].1);
    }

    #[test]
    fn line_exactly_at_cap_is_not_truncated() {
        let mut data = vec![b'y'; MAX_LINE_BYTES];
        data.push(b'\n');
        let l = lines(&data);
        assert!(!l[0].1);
        assert_eq!(l[0].0.len(), MAX_LINE_BYTES);
    }
}
