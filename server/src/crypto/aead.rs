use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::Context;
use base64::{engine::general_purpose::STANDARD, Engine};

#[derive(Clone)]
pub struct Aead256 {
    cipher: Aes256Gcm,
}

impl Aead256 {
    pub fn from_base64_key(base64_key: &str) -> anyhow::Result<Self> {
        let bytes = STANDARD
            .decode(base64_key)
            .context("TOKEN_ENC_KEY must be valid base64")?;
        anyhow::ensure!(bytes.len() == 32, "TOKEN_ENC_KEY must decode to 32 bytes");

        let key = Key::<Aes256Gcm>::from_slice(&bytes);
        Ok(Self {
            cipher: Aes256Gcm::new(key),
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| anyhow::anyhow!("encrypt_failed"))?;

        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn decrypt(&self, blob: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!(blob.len() >= 12 + 16, "ciphertext_too_short");
        let (nonce, ciphertext) = blob.split_at(12);

        self.cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow::anyhow!("decrypt_failed"))
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine};

    use super::Aead256;

    fn key(bytes: u8) -> String {
        STANDARD.encode([bytes; 32])
    }

    #[test]
    fn round_trip_encrypt_decrypt() {
        let aead = Aead256::from_base64_key(&key(7)).expect("aead");
        let encrypted = aead.encrypt(b"secret-token").expect("encrypt");
        let decrypted = aead.decrypt(&encrypted).expect("decrypt");

        assert_eq!(decrypted, b"secret-token");
        assert_ne!(encrypted, b"secret-token");
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let aead_1 = Aead256::from_base64_key(&key(7)).expect("aead1");
        let aead_2 = Aead256::from_base64_key(&key(9)).expect("aead2");
        let encrypted = aead_1.encrypt(b"secret-token").expect("encrypt");

        let err = aead_2
            .decrypt(&encrypted)
            .expect_err("wrong key should fail");
        assert!(err.to_string().contains("decrypt_failed"));
    }
}
