use crate::error::{AppError, Result};
pub const MIN_KBPS: u32 = 64;
pub const MAX_KBPS: u32 = 10_000_000;
pub fn apply(_: u32) -> Result<()> {
    Err(AppError::Invalid(
        "Bandwidth limits are unavailable on macOS.".into(),
    ))
}
pub fn boot_ms(_: i64) -> i64 {
    0
}
pub fn applied_this_boot(_: i64, _: i64) -> bool {
    false
}

pub fn script(_: u32) -> Result<String> {
    Err(AppError::Invalid(
        "Bandwidth limits are unavailable on macOS.".into(),
    ))
}
