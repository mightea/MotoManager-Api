use crate::error::{AppError, AppResult};
use rand::RngCore;
use scrypt::{
    password_hash::rand_core::OsRng as ScryptOsRng,
    scrypt, Params,
};

/// Verify a password against the stored hash.
/// Format: `{salt_hex}:{dk_hex}` where dk is 64 bytes encoded as hex.
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parts: Vec<&str> = hash.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest("Invalid password hash format".to_string()));
    }

    let salt_bytes = hex::decode(parts[0])
        .map_err(|_| AppError::Internal("Failed to decode salt".to_string()))?;
    let dk_bytes = hex::decode(parts[1])
        .map_err(|_| AppError::Internal("Failed to decode derived key".to_string()))?;

    // Node.js scrypt: N=16384, r=8, p=1, keylen=64
    let params = Params::new(14, 8, 1, 64)
        .map_err(|e| AppError::Internal(format!("Scrypt params error: {}", e)))?;

    let mut derived = vec![0u8; 64];
    scrypt(password.as_bytes(), &salt_bytes, &params, &mut derived)
        .map_err(|e| AppError::Internal(format!("Scrypt error: {}", e)))?;

    // Constant-time comparison
    Ok(constant_time_eq(&derived, &dk_bytes))
}

/// Hash a new password in the same format as Node.js scrypt.
pub fn hash_password(password: &str) -> AppResult<String> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);

    let params = Params::new(14, 8, 1, 64)
        .map_err(|e| AppError::Internal(format!("Scrypt params error: {}", e)))?;

    let mut derived = vec![0u8; 64];
    scrypt(password.as_bytes(), &salt, &params, &mut derived)
        .map_err(|e| AppError::Internal(format!("Scrypt error: {}", e)))?;

    Ok(format!("{}:{}", hex::encode(salt), hex::encode(derived)))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

// Suppress unused import warning - ScryptOsRng is needed by scrypt crate
const _: () = {
    let _ = std::mem::size_of::<ScryptOsRng>();
};
