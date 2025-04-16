use aes_gcm::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Error, Nonce};

pub fn encrept_file(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key);

    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes); // Secure RNG
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())?;

    Ok([&nonce_bytes[..], &ciphertext[..]].concat())
}

pub fn decrypt_file(key: &[u8], data: &[u8]) -> Result<Vec<u8>, Error> {
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key);

    let cipher = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    Ok(cipher.decrypt(nonce, ciphertext)?)
}
