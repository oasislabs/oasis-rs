//! Utility module

pub type Hash = [u8; 32];

/// Converts u32 to right aligned array of 32 bytes.
pub fn pad_u32(value: u32) -> Hash {
    let mut padded = [0u8; 32];
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}

/// Converts u64 to right aligned array of 32 bytes.
pub fn pad_u64(value: u64) -> Hash {
    let mut padded = [0u8; 32];
    padded[24] = (value >> 56) as u8;
    padded[25] = (value >> 48) as u8;
    padded[26] = (value >> 40) as u8;
    padded[27] = (value >> 32) as u8;
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}

/// Converts i64 to right aligned array of 32 bytes.
pub fn pad_i64(value: i64) -> Hash {
    if value >= 0 {
        return pad_u64(value as u64);
    }

    let mut padded = [0xffu8; 32];
    padded[24] = (value >> 56) as u8;
    padded[25] = (value >> 48) as u8;
    padded[26] = (value >> 40) as u8;
    padded[27] = (value >> 32) as u8;
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}

/// Converts i32 to right aligned array of 32 bytes.
pub fn pad_i32(value: i32) -> Hash {
    if value >= 0 {
        return pad_u32(value as u32);
    }

    let mut padded = [0xffu8; 32];
    padded[28] = (value >> 24) as u8;
    padded[29] = (value >> 16) as u8;
    padded[30] = (value >> 8) as u8;
    padded[31] = value as u8;
    padded
}
