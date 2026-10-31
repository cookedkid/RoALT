use crate::error::{AppError,Result};
use std::ffi::CString;
unsafe extern "C" {
    fn ro_biometrics_available() -> bool;
    fn ro_biometrics_verify(reason:*const std::ffi::c_char) -> bool;
}
pub async fn hello_available()->bool { unsafe{ro_biometrics_available()} }
pub async fn windows_hello(_: &tauri::WebviewWindow, message:&str)->Result<()> {
    let reason=CString::new(message).map_err(|_|AppError::Invalid("Invalid authentication prompt.".into()))?;
    if !hello_available().await{return Err(AppError::Invalid("Touch ID is unavailable. Set a master password to reveal cookies.".into()))}
    let verified=tauri::async_runtime::spawn_blocking(move||unsafe{ro_biometrics_verify(reason.as_ptr())}).await.map_err(|e|AppError::Internal(e.to_string()))?;
    if verified{Ok(())}else{Err(AppError::Invalid("Touch ID verification was cancelled or unsuccessful.".into()))}
}
