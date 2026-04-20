#[cfg(test)]
mod tests {
    use crate::auth::{generate_device_keypair, verify_message, UserSession};
    use ed25519_dalek::Signer;

    #[test]
    fn test_create_session() {
        let device_id = uuid::Uuid::new_v4();
        let session = UserSession::new(
            "user_123".into(),
            device_id,
            "auth_token_xyz".into(),
            vec![0u8; 32],
        );
        assert_eq!(session.user_id, "user_123");
        assert_eq!(session.device_id, device_id);
    }

    #[test]
    fn test_generate_device_keypair() {
        let (public_key, secret_key) = generate_device_keypair();
        assert_eq!(public_key.to_bytes().len(), 32);
        assert_eq!(secret_key.to_bytes().len(), 32);
    }

    #[test]
    fn test_sign_and_verify_message() {
        let (public_key, signing_key) = generate_device_keypair();
        let message = b"test message";
        let signature = signing_key.sign(message);
        assert!(verify_message(&public_key, message, &signature).is_ok());
    }

    #[test]
    fn test_sign_and_verify_tampered_message() {
        let (public_key, signing_key) = generate_device_keypair();
        let message = b"test message";
        let signature = signing_key.sign(message);
        // Tamper with the signature bytes
        let mut sig_bytes = signature.to_bytes();
        sig_bytes[0] ^= 0xFF;
        let tampered = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        assert!(verify_message(&public_key, message, &tampered).is_err());
    }
}
