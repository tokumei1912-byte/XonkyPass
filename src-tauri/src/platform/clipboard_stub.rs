//! Заглушка буфера обмена для сборки вне Windows (проверки в Linux).

use super::{PlatformError, PlatformResult};

pub fn write_secret(_owner: isize, _text: &str) -> PlatformResult<u32> {
    Err(PlatformError::unsupported())
}

pub fn clear_if_unchanged(_owner: isize, _seq: u32) -> PlatformResult<bool> {
    Ok(false)
}
