use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::{AppError, AppResult};

/// Hash a password using Argon2id with OWASP-recommended parameters.
/// Returns a PHC string: `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`
pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);

    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))
}

/// Verify a password against an Argon2id PHC string.
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Invalid password hash: {}", e)))?;

    Ok(argon2()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn argon2() -> Argon2<'static> {
    use argon2::{Algorithm, Params, Version};
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19456, 2, 1, None).expect("valid argon2 params"),
    )
}
