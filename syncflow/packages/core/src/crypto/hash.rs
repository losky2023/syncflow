use crate::error::Result;

/// Hash data using BLAKE3 and return hex string.
pub fn hash_data(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Hash a file incrementally.
pub fn hash_file_content(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_data_deterministic() {
        let data = b"test file content";
        let hash1 = hash_data(data);
        let hash2 = hash_data(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_data_different_content() {
        let hash1 = hash_data(b"content A");
        let hash2 = hash_data(b"content B");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_data_empty() {
        let hash = hash_data(b"");
        assert!(!hash.is_empty());
        // BLAKE3 of empty input has a known hash
        assert_eq!(
            hash,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }
}
