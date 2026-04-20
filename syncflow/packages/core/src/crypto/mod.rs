pub mod encrypt;
pub mod hash;
pub mod key_derive;

#[cfg(test)]
mod tests;

pub use encrypt::{decrypt_data, encrypt_data};
pub use hash::{hash_data, hash_file_content};
pub use key_derive::derive_root_key;
