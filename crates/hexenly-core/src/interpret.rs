#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteClass {
    Null,
    MaxByte,
    PrintableAscii,
    Other,
}

pub fn classify_byte(b: u8) -> ByteClass {
    match b {
        0x00 => ByteClass::Null,
        0xFF => ByteClass::MaxByte,
        0x20..=0x7E => ByteClass::PrintableAscii,
        _ => ByteClass::Other,
    }
}

#[derive(Debug, Clone)]
pub struct Interpretation {
    pub byte: u8,
    pub hex: String,
    pub decimal: String,
    pub octal: String,
    pub binary: String,
    pub ascii: Option<char>,

    // Little-endian interpretations (require enough bytes)
    pub u16_le: Option<u16>,
    pub u32_le: Option<u32>,
    pub u64_le: Option<u64>,
    pub i16_le: Option<i16>,
    pub i32_le: Option<i32>,
    pub i64_le: Option<i64>,
    pub f32_le: Option<f32>,
    pub f64_le: Option<f64>,

    // Big-endian interpretations
    pub u16_be: Option<u16>,
    pub u32_be: Option<u32>,
    pub u64_be: Option<u64>,
    pub i16_be: Option<i16>,
    pub i32_be: Option<i32>,
    pub i64_be: Option<i64>,
    pub f32_be: Option<f32>,
    pub f64_be: Option<f64>,

    // Date/time interpretations
    pub unix_ts_u32_le: Option<String>,
    pub unix_ts_u32_be: Option<String>,
    pub unix_ts_u64_le: Option<String>,
    pub unix_ts_u64_be: Option<String>,
    pub dos_datetime_le: Option<String>,
    pub dos_datetime_be: Option<String>,
    pub filetime_le: Option<String>,

    // Text interpretations
    pub utf8_char: Option<String>,
    pub utf16_le_char: Option<String>,
    pub utf16_be_char: Option<String>,
}

pub struct ByteInterpreter;

impl ByteInterpreter {
    pub fn interpret(data: &[u8], offset: usize) -> Option<Interpretation> {
        let byte = *data.get(offset)?;
        let remaining = &data[offset..];

        let ascii = if (0x20..=0x7E).contains(&byte) {
            Some(byte as char)
        } else {
            None
        };

        let u16_le = try_read::<2>(remaining).map(u16::from_le_bytes);
        let u32_le = try_read::<4>(remaining).map(u32::from_le_bytes);
        let u64_le = try_read::<8>(remaining).map(u64::from_le_bytes);
        let i16_le = try_read::<2>(remaining).map(i16::from_le_bytes);
        let i32_le = try_read::<4>(remaining).map(i32::from_le_bytes);
        let i64_le = try_read::<8>(remaining).map(i64::from_le_bytes);
        let f32_le = try_read::<4>(remaining).map(f32::from_le_bytes);
        let f64_le = try_read::<8>(remaining).map(f64::from_le_bytes);

        let u16_be = try_read::<2>(remaining).map(u16::from_be_bytes);
        let u32_be = try_read::<4>(remaining).map(u32::from_be_bytes);
        let u64_be = try_read::<8>(remaining).map(u64::from_be_bytes);
        let i16_be = try_read::<2>(remaining).map(i16::from_be_bytes);
        let i32_be = try_read::<4>(remaining).map(i32::from_be_bytes);
        let i64_be = try_read::<8>(remaining).map(i64::from_be_bytes);
        let f32_be = try_read::<4>(remaining).map(f32::from_be_bytes);
        let f64_be = try_read::<8>(remaining).map(f64::from_be_bytes);

        // Date/time interpretations
        let unix_ts_u32_le = u32_le.map(|v| format_unix_timestamp(v as u64));
        let unix_ts_u32_be = u32_be.map(|v| format_unix_timestamp(v as u64));
        let unix_ts_u64_le = u64_le.map(format_unix_timestamp);
        let unix_ts_u64_be = u64_be.map(format_unix_timestamp);

        let dos_datetime_le = if remaining.len() >= 4 {
            let time_val = u16::from_le_bytes([remaining[0], remaining[1]]);
            let date_val = u16::from_le_bytes([remaining[2], remaining[3]]);
            Some(format_dos_datetime(time_val, date_val))
        } else {
            None
        };
        let dos_datetime_be = if remaining.len() >= 4 {
            let time_val = u16::from_be_bytes([remaining[0], remaining[1]]);
            let date_val = u16::from_be_bytes([remaining[2], remaining[3]]);
            Some(format_dos_datetime(time_val, date_val))
        } else {
            None
        };
        let filetime_le = u64_le.and_then(format_filetime);

        // Text interpretations
        let utf8_char = decode_utf8_char(remaining);
        let utf16_le_char = decode_utf16_char(remaining, false);
        let utf16_be_char = decode_utf16_char(remaining, true);

        Some(Interpretation {
            byte,
            hex: format!("{byte:02X}"),
            decimal: format!("{byte}"),
            octal: format!("{byte:03o}"),
            binary: format!("{byte:08b}"),
            ascii,
            u16_le,
            u32_le,
            u64_le,
            i16_le,
            i32_le,
            i64_le,
            f32_le,
            f64_le,
            u16_be,
            u32_be,
            u64_be,
            i16_be,
            i32_be,
            i64_be,
            f32_be,
            f64_be,
            unix_ts_u32_le,
            unix_ts_u32_be,
            unix_ts_u64_le,
            unix_ts_u64_be,
            dos_datetime_le,
            dos_datetime_be,
            filetime_le,
            utf8_char,
            utf16_le_char,
            utf16_be_char,
        })
    }
}

fn try_read<const N: usize>(data: &[u8]) -> Option<[u8; N]> {
    if data.len() < N {
        return None;
    }
    let mut buf = [0u8; N];
    buf.copy_from_slice(&data[..N]);
    Some(buf)
}

// ── Date/time helpers ────────────────────────────────────────────

/// Howard Hinnant's civil_from_days algorithm.
/// Converts a day count from Unix epoch (1970-01-01) to (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a Unix timestamp (seconds since 1970-01-01) as "YYYY-MM-DD HH:MM:SS UTC".
fn format_unix_timestamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let day_secs = secs % 86400;
    let (y, m, d) = civil_from_days(days);
    let h = day_secs / 3600;
    let min = (day_secs % 3600) / 60;
    let s = day_secs % 60;
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02} UTC")
}

/// Format a DOS date/time pair as "YYYY-MM-DD HH:MM:SS".
fn format_dos_datetime(time_u16: u16, date_u16: u16) -> String {
    let second = (time_u16 & 0x1F) * 2;
    let minute = (time_u16 >> 5) & 0x3F;
    let hour = (time_u16 >> 11) & 0x1F;
    let day = date_u16 & 0x1F;
    let month = (date_u16 >> 5) & 0x0F;
    let year = ((date_u16 >> 9) & 0x7F) + 1980;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// Format a Windows FILETIME (100-ns ticks since 1601-01-01) as a UTC string.
fn format_filetime(ft: u64) -> Option<String> {
    // FILETIME epoch is 1601-01-01, Unix epoch is 1970-01-01.
    // Difference is 11644473600 seconds = 116444736000000000 hundred-nanoseconds.
    const EPOCH_DIFF: u64 = 116_444_736_000_000_000;
    if ft < EPOCH_DIFF {
        return None;
    }
    let unix_100ns = ft - EPOCH_DIFF;
    let secs = unix_100ns / 10_000_000;
    Some(format_unix_timestamp(secs))
}

/// Decode a UTF-8 character at the start of data, returning "U+XXXX 'c' (N bytes)".
fn decode_utf8_char(data: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(data).ok().or_else(|| {
        // Try just the first 1-4 bytes
        for len in (1..=4.min(data.len())).rev() {
            if let Ok(s) = std::str::from_utf8(&data[..len]) {
                return Some(s);
            }
        }
        None
    })?;
    let ch = s.chars().next()?;
    let byte_len = ch.len_utf8();
    Some(format!("U+{:04X} '{}' ({byte_len}B)", ch as u32, ch))
}

/// Decode a UTF-16 character (LE or BE) at the start of data.
fn decode_utf16_char(data: &[u8], big_endian: bool) -> Option<String> {
    if data.len() < 2 {
        return None;
    }
    let read_u16 = |d: &[u8]| -> u16 {
        if big_endian {
            u16::from_be_bytes([d[0], d[1]])
        } else {
            u16::from_le_bytes([d[0], d[1]])
        }
    };
    let first = read_u16(data);
    // Check for surrogate pair
    if (0xD800..=0xDBFF).contains(&first) {
        if data.len() < 4 {
            return None;
        }
        let second = read_u16(&data[2..]);
        if !(0xDC00..=0xDFFF).contains(&second) {
            return None;
        }
        let cp = 0x10000 + ((first as u32 - 0xD800) << 10) + (second as u32 - 0xDC00);
        let ch = char::from_u32(cp)?;
        Some(format!("U+{cp:04X} '{ch}' (4B)"))
    } else if (0xDC00..=0xDFFF).contains(&first) {
        None // lone low surrogate
    } else {
        let ch = char::from_u32(first as u32)?;
        Some(format!("U+{:04X} '{ch}' (2B)", first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_known_date() {
        // 2024-01-15 is 19737 days from epoch
        assert_eq!(civil_from_days(19_737), (2024, 1, 15));
    }

    #[test]
    fn format_unix_timestamp_epoch() {
        assert_eq!(format_unix_timestamp(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn format_unix_timestamp_known() {
        // 2024-01-15 13:45:22 UTC = 1705326322
        assert_eq!(
            format_unix_timestamp(1_705_326_322),
            "2024-01-15 13:45:22 UTC"
        );
    }

    #[test]
    fn format_dos_datetime_known() {
        // DOS date: 2024-01-15 => year_offset=44, month=1, day=15
        // date_u16 = (44 << 9) | (1 << 5) | 15 = 22575
        // DOS time: 13:45:22 => hour=13, minute=45, second=22/2=11
        // time_u16 = (13 << 11) | (45 << 5) | 11 = 28_075
        let result = format_dos_datetime(28_075, 22_575);
        assert_eq!(result, "2024-01-15 13:45:22");
    }

    #[test]
    fn format_filetime_known() {
        // 2024-01-15 13:45:22 UTC as FILETIME
        // Unix secs = 1705326322
        // FILETIME = (1705326322 + 11644473600) * 10_000_000
        let ft = (1_705_326_322u64 + 11_644_473_600) * 10_000_000;
        assert_eq!(
            format_filetime(ft),
            Some("2024-01-15 13:45:22 UTC".to_string())
        );
    }

    #[test]
    fn format_filetime_before_epoch() {
        assert_eq!(format_filetime(0), None);
    }

    #[test]
    fn decode_utf8_ascii() {
        assert_eq!(
            decode_utf8_char(b"A"),
            Some("U+0041 'A' (1B)".to_string())
        );
    }

    #[test]
    fn decode_utf8_multibyte() {
        // Euro sign: U+20AC, encoded as E2 82 AC
        let data = [0xE2, 0x82, 0xAC];
        let result = decode_utf8_char(&data).unwrap();
        assert!(result.contains("U+20AC"));
        assert!(result.contains("(3B)"));
    }

    #[test]
    fn decode_utf8_invalid() {
        assert_eq!(decode_utf8_char(&[0xFF, 0xFE]), None);
    }

    #[test]
    fn decode_utf16_le_basic() {
        // 'A' = 0x0041, LE = [0x41, 0x00]
        assert_eq!(
            decode_utf16_char(&[0x41, 0x00], false),
            Some("U+0041 'A' (2B)".to_string())
        );
    }

    #[test]
    fn decode_utf16_be_basic() {
        // 'A' = 0x0041, BE = [0x00, 0x41]
        assert_eq!(
            decode_utf16_char(&[0x00, 0x41], true),
            Some("U+0041 'A' (2B)".to_string())
        );
    }

    #[test]
    fn decode_utf16_surrogate_pair_le() {
        // U+1F600 (grinning face) = surrogate pair D83D DE00
        // LE: [0x3D, 0xD8, 0x00, 0xDE]
        let result = decode_utf16_char(&[0x3D, 0xD8, 0x00, 0xDE], false).unwrap();
        assert!(result.contains("U+1F600"));
        assert!(result.contains("(4B)"));
    }

    #[test]
    fn decode_utf16_lone_surrogate() {
        // Lone low surrogate: 0xDC00 in BE = [0xDC, 0x00]
        assert_eq!(decode_utf16_char(&[0xDC, 0x00], true), None);
    }

    #[test]
    fn decode_utf16_too_short() {
        assert_eq!(decode_utf16_char(&[0x41], false), None);
    }

    #[test]
    fn interpret_populates_datetime_fields() {
        // 4 bytes: enough for u32 timestamps and DOS datetime
        let data = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let interp = ByteInterpreter::interpret(&data, 0).unwrap();
        assert!(interp.unix_ts_u32_le.is_some());
        assert!(interp.unix_ts_u32_be.is_some());
        assert!(interp.dos_datetime_le.is_some());
        assert!(interp.dos_datetime_be.is_some());
        assert!(interp.unix_ts_u64_le.is_some());
        // null byte is valid UTF-8 (U+0000)
        assert!(interp.utf8_char.is_some());
    }

    #[test]
    fn interpret_text_fields() {
        let data = b"Hello world";
        let interp = ByteInterpreter::interpret(data, 0).unwrap();
        assert_eq!(interp.utf8_char, Some("U+0048 'H' (1B)".to_string()));
        assert!(interp.utf16_le_char.is_some());
    }
}
