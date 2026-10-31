//! One Keychain master key wraps independent authenticated vault and database blobs.
use crate::error::{AppError, Result};
#[cfg(not(test))]
use security_framework::passwords::{get_generic_password, set_generic_password};
use zeroize::Zeroizing;
pub const ENTROPY: &[u8] = b"RoALT/vault/v1";
#[cfg(not(test))]
const SERVICE: &str = "com.inspectsoftware.roalt";
#[cfg(not(test))]
const ACCOUNT: &str = "wrapping-key-v2";
#[cfg(not(test))]
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(not(test))]
fn key(create: bool) -> Result<Zeroizing<[u8; 32]>> {
    let _lock = LOCK.lock().map_err(|_| AppError::Crypto)?;
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            Ok(Zeroizing::new(
                bytes.as_slice().try_into().map_err(|_| AppError::Crypto)?,
            ))
        }
        Err(e) if e.code() == -25300 && create => {
            let key = Zeroizing::new(super::master::random_bytes::<32>());
            set_generic_password(SERVICE, ACCOUNT, key.as_ref()).map_err(|_| AppError::Crypto)?;
            Ok(key)
        }
        Err(_) => Err(AppError::Crypto),
    }
}
// Unit tests must never access the user's production wrapping key or its ACL.
// Keychain itself is exercised separately with a fresh test-only item below.
#[cfg(test)]
fn key(_:bool)->Result<Zeroizing<[u8;32]>> {
    static KEY:std::sync::OnceLock<Zeroizing<[u8;32]>>=std::sync::OnceLock::new();
    Ok(Zeroizing::new(**KEY.get_or_init(||Zeroizing::new(super::master::random_bytes::<32>()))))
}
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    protect_with(data, ENTROPY)
}
pub fn unprotect(data: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    unprotect_with(data, ENTROPY)
}
pub fn protect_with(data: &[u8], entropy: &[u8]) -> Result<Vec<u8>> {
    let key = key(true)?;
    let mut out = b"ROALT-MAC-2".to_vec();
    out.extend(super::master::seal(&key, entropy, data)?);
    Ok(out)
}
pub fn unprotect_with(blob: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let blob = blob.strip_prefix(b"ROALT-MAC-2").ok_or(AppError::Crypto)?;
    super::master::open(&*key(false)?, entropy, blob)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keychain_round_trip_uses_disposable_test_item() {
        use security_framework::passwords::{get_generic_password,set_generic_password,delete_generic_password};
        const SERVICE:&str="com.inspectsoftware.roalt.tests";
        let account=uuid::Uuid::new_v4().to_string();
        let key=Zeroizing::new(super::super::master::random_bytes::<32>());
        set_generic_password(SERVICE,&account,key.as_ref()).unwrap();
        let read=get_generic_password(SERVICE,&account).map(Zeroizing::new);
        let deleted=delete_generic_password(SERVICE,&account);
        assert_eq!(read.unwrap().as_slice(),key.as_ref());
        deleted.unwrap();
    }
    #[test]
    fn independent_blobs_and_authentication() {
        let key = super::super::master::random_bytes::<32>();
        let a = super::super::master::seal(&key, b"a", b"first").unwrap();
        let b = super::super::master::seal(&key, b"a", b"second").unwrap();
        assert_eq!(
            &*super::super::master::open(&key, b"a", &a).unwrap(),
            b"first"
        );
        assert_eq!(
            &*super::super::master::open(&key, b"a", &b).unwrap(),
            b"second"
        );
        assert!(super::super::master::open(&key, b"b", &a).is_err());
        let mut corrupt = a;
        corrupt[14] ^= 1;
        assert!(super::super::master::open(&key, b"a", &corrupt).is_err());
    }
}
