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
