#[cfg(test)]
mod tests {
    use crate::crypto::{decrypt_data, derive_root_key, encrypt_data};

    #[test]
    fn test_derive_root_key_produces_32_bytes() {
        let salt = b"test_salt_16byte";
        let root_key = derive_root_key("my_secure_password", salt).unwrap();
        assert_eq!(root_key.len(), 32);
    }

    #[test]
    fn test_derive_root_key_deterministic() {
        let salt = b"test_salt_16byte";
        let key1 = derive_root_key("my_secure_password", salt).unwrap();
        let key2 = derive_root_key("my_secure_password", salt).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_derive_root_key_different_passwords() {
        let salt = b"test_salt_16byte";
        let key1 = derive_root_key("password1", salt).unwrap();
        let key2 = derive_root_key("password2", salt).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0u8; 32];
        let plaintext = b"Hello, SyncFlow!";
        let encrypted = encrypt_data(plaintext, &key).unwrap();
        assert!(encrypted.len() > plaintext.len());
        let decrypted = decrypt_data(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_wrong_key() {
        let key1 = [0u8; 32];
        let key2 = [1u8; 32];
        let plaintext = b"Secret data";
        let encrypted = encrypt_data(plaintext, &key1).unwrap();
        let result = decrypt_data(&encrypted, &key2);
        assert!(result.is_err());
    }
}
