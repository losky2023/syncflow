use crate::error::{Result, SyncFlowError};
use argon2::{Argon2, Params};

/// Derive a 32-byte root key from password + salt using Argon2id.
/// Parameters: 64 MiB memory, 3 iterations, 4 parallelism.
pub fn derive_root_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    if salt.len() < 16 {
        return Err(SyncFlowError::Crypto(
            "Salt must be at least 16 bytes".into(),
        ));
    }

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        Params::new(
            64 * 1024, // 64 MiB
            3,         // 3 iterations
            4,         // 4 parallelism
            Some(32),  // 32-byte output
        )
        .map_err(|e| SyncFlowError::Crypto(format!("Argon2 params error: {}", e)))?,
    );

    let mut output_key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut output_key)
        .map_err(|e| SyncFlowError::Crypto(format!("Argon2 hashing error: {}", e)))?;

    Ok(output_key)
}
