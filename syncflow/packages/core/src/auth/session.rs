use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use secrecy::SecretBox;
use uuid::Uuid;

/// Authenticated user session.
pub struct UserSession {
    pub user_id: String,
    pub device_id: Uuid,
    pub auth_token: String,
    pub root_key: SecretBox<[u8; 32]>,
}

impl UserSession {
    pub fn new(user_id: String, device_id: Uuid, auth_token: String, root_key: Vec<u8>) -> Self {
        Self {
            user_id,
            device_id,
            auth_token,
            root_key: SecretBox::new(root_key.try_into().expect("root key must be 32 bytes")),
        }
    }
}

/// Generate an Ed25519 device signing keypair.
pub fn generate_device_keypair() -> (VerifyingKey, SigningKey) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (verifying_key, signing_key)
}

/// Sign a message with the device signing key.
pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> ed25519_dalek::Signature {
    signing_key.sign(message)
}

/// Verify a message signature with the device verifying key.
pub fn verify_message(
    verifying_key: &VerifyingKey,
    message: &[u8],
    signature: &ed25519_dalek::Signature,
) -> Result<(), ed25519_dalek::SignatureError> {
    verifying_key.verify_strict(message, signature)
}
