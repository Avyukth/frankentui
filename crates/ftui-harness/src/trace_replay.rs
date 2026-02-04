#![forbid(unsafe_code)]

//! Render-trace replay harness.
//!
//! Replays render-trace v1 JSONL logs into a deterministic buffer model,
//! verifies per-frame checksums, and reports mismatches with clear diagnostics.
//!
//! Designed for CI use: non-interactive, bounded, and deterministic.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde_json::Value;

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone)]
enum TraceContent {
    Empty,
    Char(u32),
    Grapheme(Vec<u8>),
    Continuation,
}

impl TraceContent {
    fn kind(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Char(_) => 1,
            Self::Grapheme(_) => 2,
            Self::Continuation => 3,
        }
    }
}

#[derive(Debug, Clone)]
struct TraceCell {
    content: TraceContent,
    fg: u32,
    bg: u32,
    attrs: u32,
}

impl Default for TraceCell {
    fn default() -> Self {
        Self {
            content: TraceContent::Empty,
            fg: ftui_render::cell::PackedRgba::WHITE.0,
            bg: ftui_render::cell::PackedRgba::TRANSPARENT.0,
            attrs: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct TraceGrid {
    width: u16,
    height: u16,
    cells: Vec<TraceCell>,
}

impl TraceGrid {
    fn new(width: u16, height: u16) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![TraceCell::default(); len],
        }
    }

    fn resize(&mut self, width: u16, height: u16) {
        *self = Self::new(width, height);
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    fn set_cell(&mut self, x: u16, y: u16, cell: TraceCell) -> io::Result<()> {
        let idx = self
            .index(x, y)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "cell out of bounds"))?;
        self.cells[idx] = cell;
        Ok(())
    }

    fn checksum(&self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        for cell in &self.cells {
            let kind = cell.content.kind();
            fnv1a_update(&mut hash, &[kind]);
            match &cell.content {
                TraceContent::Empty | TraceContent::Continuation => {
                    fnv1a_update(&mut hash, &0u16.to_le_bytes());
                }
                TraceContent::Char(codepoint) => {
                    let ch = char::from_u32(*codepoint).unwrap_or('\u{FFFD}');
                    let mut buf = [0u8; 4];
                    let encoded = ch.encode_utf8(&mut buf);
                    let bytes = encoded.as_bytes();
                    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
                    fnv1a_update(&mut hash, &len.to_le_bytes());
                    fnv1a_update(&mut hash, &bytes[..len as usize]);
                }
                TraceContent::Grapheme(bytes) => {
                    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
                    fnv1a_update(&mut hash, &len.to_le_bytes());
                    fnv1a_update(&mut hash, &bytes[..len as usize]);
                }
            }
            fnv1a_update(&mut hash, &cell.fg.to_le_bytes());
            fnv1a_update(&mut hash, &cell.bg.to_le_bytes());
            fnv1a_update(&mut hash, &cell.attrs.to_le_bytes());
        }
        hash
    }

    fn apply_diff_runs(&mut self, payload: &[u8]) -> io::Result<ApplyStats> {
        let mut cursor = io::Cursor::new(payload);
        let width = read_u16(&mut cursor)?;
        let height = read_u16(&mut cursor)?;
        let run_count = read_u32(&mut cursor)? as usize;

        if width != self.width || height != self.height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload dimensions do not match frame dimensions",
            ));
        }

        let mut cells_applied = 0usize;
        for _ in 0..run_count {
            let y = read_u16(&mut cursor)?;
            let x0 = read_u16(&mut cursor)?;
            let x1 = read_u16(&mut cursor)?;
            if x1 < x0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid run range",
                ));
            }
            if y >= self.height || x1 >= self.width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "run out of bounds",
                ));
            }
            for x in x0..=x1 {
                let cell = read_cell(&mut cursor)?;
                self.set_cell(x, y, cell)?;
                cells_applied += 1;
            }
        }

        if cursor.position() as usize != payload.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload has trailing bytes",
            ));
        }

        Ok(ApplyStats {
            runs: run_count,
            cells: cells_applied,
        })
    }

    fn apply_full_buffer(&mut self, payload: &[u8]) -> io::Result<ApplyStats> {
        let mut cursor = io::Cursor::new(payload);
        let width = read_u16(&mut cursor)?;
        let height = read_u16(&mut cursor)?;
        if width != self.width || height != self.height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload dimensions do not match frame dimensions",
            ));
        }

        let mut cells_applied = 0usize;
        for y in 0..height {
            for x in 0..width {
                let cell = read_cell(&mut cursor)?;
                self.set_cell(x, y, cell)?;
                cells_applied += 1;
            }
        }

        if cursor.position() as usize != payload.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload has trailing bytes",
            ));
        }

        Ok(ApplyStats {
            runs: height as usize,
            cells: cells_applied,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ApplyStats {
    runs: usize,
    cells: usize,
}

/// Result summary for a replay run.
#[derive(Debug, Clone)]
pub struct ReplaySummary {
    pub frames: usize,
    pub last_checksum: Option<u64>,
}

/// Replay a render-trace JSONL file and verify per-frame checksums.
pub fn replay_trace(path: impl AsRef<Path>) -> io::Result<ReplaySummary> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

    let mut grid = TraceGrid::new(0, 0);
    let mut frames = 0usize;
    let mut last_checksum = None;

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid JSONL at line {}: {err}", line_idx + 1),
            )
        })?;
        let Some(event) = value.get("event").and_then(Value::as_str) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing event at line {}", line_idx + 1),
            ));
        };
        if event != "frame" {
            continue;
        }

        let frame_idx = parse_u64(&value, "frame_idx")?;
        let cols = parse_u16(&value, "cols")?;
        let rows = parse_u16(&value, "rows")?;
        let payload_kind = parse_str(&value, "payload_kind")?;
        let payload_path =
            parse_optional_str(&value, "payload_path").map(|p| resolve_payload_path(base_dir, &p));
        let expected_checksum = parse_hex_u64(parse_str(&value, "checksum")?)?;

        if grid.width != cols || grid.height != rows {
            grid.resize(cols, rows);
        }

        let stats = match payload_kind {
            "diff_runs_v1" => {
                let payload_path = payload_path.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "payload_path missing")
                })?;
                let payload = std::fs::read(&payload_path)?;
                grid.apply_diff_runs(&payload)?
            }
            "full_buffer_v1" => {
                let payload_path = payload_path.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "payload_path missing")
                })?;
                let payload = std::fs::read(&payload_path)?;
                grid.apply_full_buffer(&payload)?
            }
            "none" => ApplyStats { runs: 0, cells: 0 },
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported payload_kind {other} at frame {frame_idx}"),
                ));
            }
        };

        let actual_checksum = grid.checksum();
        if actual_checksum != expected_checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "checksum mismatch at frame {}: expected {:016x}, got {:016x} (payload_kind={}, runs={}, cells={})",
                    frame_idx,
                    expected_checksum,
                    actual_checksum,
                    payload_kind,
                    stats.runs,
                    stats.cells
                ),
            ));
        }

        frames += 1;
        last_checksum = Some(actual_checksum);
    }

    if frames == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no frame records found",
        ));
    }

    Ok(ReplaySummary {
        frames,
        last_checksum,
    })
}

fn resolve_payload_path(base_dir: &Path, payload: &str) -> PathBuf {
    let payload_path = Path::new(payload);
    if payload_path.is_absolute() {
        payload_path.to_path_buf()
    } else {
        base_dir.join(payload_path)
    }
}

fn parse_u64(value: &Value, field: &str) -> io::Result<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {field}")))
}

fn parse_u16(value: &Value, field: &str) -> io::Result<u16> {
    let v = parse_u64(value, field)?;
    u16::try_from(v)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("{field} out of range")))
}

fn parse_str<'a>(value: &'a Value, field: &str) -> io::Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {field}")))
}

fn parse_optional_str(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn parse_hex_u64(value: &str) -> io::Result<u64> {
    let trimmed = value.trim().trim_start_matches("0x");
    if trimmed.len() != 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("checksum must be 16 hex chars, got {value}"),
        ));
    }
    u64::from_str_radix(trimmed, 16).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid checksum {value}: {err}"),
        )
    })
}

fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn read_u8<R: Read>(reader: &mut R) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<R: Read>(reader: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_cell<R: Read>(reader: &mut R) -> io::Result<TraceCell> {
    let kind = read_u8(reader)?;
    let content = match kind {
        0 => TraceContent::Empty,
        1 => {
            let codepoint = read_u32(reader)?;
            if char::from_u32(codepoint).is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid char codepoint {codepoint}"),
                ));
            }
            TraceContent::Char(codepoint)
        }
        2 => {
            let len = read_u16(reader)? as usize;
            if len > 4096 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "grapheme length exceeds 4096",
                ));
            }
            let mut bytes = vec![0u8; len];
            reader.read_exact(&mut bytes)?;
            TraceContent::Grapheme(bytes)
        }
        3 => TraceContent::Continuation,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid content_kind {kind}"),
            ));
        }
    };
    let fg = read_u32(reader)?;
    let bg = read_u32(reader)?;
    let attrs = read_u32(reader)?;
    Ok(TraceCell {
        content,
        fg,
        bg,
        attrs,
    })
}
