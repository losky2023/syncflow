use crate::error::{Result, SyncFlowError};
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305};

/// Encrypt data using XChaCha20-Poly1305.
/// Returns: nonce (24 bytes) || ciphertext || tag
pub fn encrypt_data(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| SyncFlowError::Crypto(format!("Encryption failed: {}", e)))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt data encrypted with encrypt_data.
/// Input format: nonce (24 bytes) || ciphertext || tag
pub fn decrypt_data(nonce_and_ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key));

    if nonce_and_ciphertext.len() < 24 {
        return Err(SyncFlowError::Crypto(
            "Invalid ciphertext: too short".into(),
        ));
    }

    let (nonce, ciphertext) = nonce_and_ciphertext.split_at(24);
    let plaintext = cipher
        .decrypt(nonce.into(), ciphertext)
        .map_err(|e| SyncFlowError::Crypto(format!("Decryption failed: {}", e)))?;

    Ok(plaintext)
}
