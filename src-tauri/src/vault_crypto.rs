//! Pure cryptographic primitives for vault encryption - key derivation,
//! authenticated encryption, and recovery-key generation. No file I/O and
//! no app/Tauri state here on purpose: this module is the one place that
//! needs to be correct for notes to ever be decryptable again, so it stays
//! small, dependency-free of the rest of the app, and thoroughly tested.
//!
//! Key hierarchy: a random 256-bit Vault Key (VK) is what actually encrypts
//! note content. The VK itself is never stored - only two *wrapped* copies
//! of it are: one encrypted under a key derived from the user's password via
//! Argon2id, another encrypted directly under a random recovery key. Either
//! secret unwraps the same VK; losing both means the notes are permanently
//! unreadable, by design (see `vault.rs` for where these wrapped copies are
//! persisted).

use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, Generate, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};

pub type CryptoResult<T> = Result<T, String>;

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
/// XChaCha20-Poly1305's extended nonce is large enough that generating it
/// randomly (rather than with a counter) carries no practical collision
/// risk, even across a vault's entire lifetime of saves.
const NONCE_LEN: usize = 24;

/// Derives a 32-byte key from a password and salt via Argon2id (RFC 9106's
/// first recommended parameters: 19 MiB, 2 iterations, 1 lane - the crate's
/// own defaults). Deterministic: the same password+salt always yields the
/// same key, which is exactly what unlocking needs.
pub fn derive_key_from_password(password: &str, salt: &[u8; SALT_LEN]) -> CryptoResult<[u8; KEY_LEN]> {
    let mut out = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn random_salt() -> [u8; SALT_LEN] {
    <[u8; SALT_LEN]>::generate()
}

/// A fresh, random 256-bit Vault Key - generated once when encryption is
/// turned on, then wrapped (never stored raw) under the password and the
/// recovery key.
pub fn random_vault_key() -> [u8; KEY_LEN] {
    <[u8; KEY_LEN]>::generate()
}

/// A recovery key is just 32 random bytes, the same shape as a Vault Key -
/// it's used directly as an AEAD key to wrap the VK, with no separate KDF
/// step (unlike the password, it's already high-entropy).
pub fn random_recovery_key() -> [u8; KEY_LEN] {
    <[u8; KEY_LEN]>::generate()
}

/// Formats a recovery key as groups of 4 uppercase hex characters
/// ("A3F2-9B01-...") - easy to read back correctly when saving/typing it,
/// unambiguous (hex has no lookalike characters to confuse), at the cost of
/// being long. It's meant to be saved once, not memorized.
pub fn format_recovery_key(key: &[u8; KEY_LEN]) -> String {
    key.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|pair| pair.concat())
        .collect::<Vec<_>>()
        .join("-")
}

/// The inverse of [`format_recovery_key`] - accepts the dashes (or their
/// absence) and is case-insensitive, so a recovery key typed back in
/// doesn't have to match its displayed formatting exactly.
pub fn parse_recovery_key(input: &str) -> CryptoResult<[u8; KEY_LEN]> {
    let hex: String = input.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if hex.len() != KEY_LEN * 2 {
        return Err("recovery key must be 64 hex characters".to_string());
    }
    let mut out = [0u8; KEY_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = &hex[i * 2..i * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).map_err(|_| "recovery key contains non-hex characters".to_string())?;
    }
    Ok(out)
}

/// Encrypts `plaintext` under `key`, returning `nonce || ciphertext` (the
/// nonce is never secret, so prepending it is the standard, simplest way to
/// keep it alongside the data it was used for).
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    let nonce = XNonce::generate();
    let ciphertext = cipher.encrypt(&nonce, plaintext).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypts data produced by [`encrypt`]. Fails (rather than returning
/// garbage) on a wrong key or any corruption/tampering, since AEAD
/// authenticates the whole ciphertext.
pub fn decrypt(key: &[u8; KEY_LEN], data: &[u8]) -> CryptoResult<Vec<u8>> {
    if data.len() < NONCE_LEN {
        return Err("encrypted data is too short to contain a nonce".to_string());
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce_array: [u8; NONCE_LEN] = nonce_bytes.try_into().expect("split_at guarantees this length");
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    let nonce = XNonce::from(nonce_array);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "failed to decrypt - wrong key, or the data is corrupted".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trips() {
        let key = random_vault_key();
        let ciphertext = encrypt(&key, b"hello, vault").unwrap();
        assert_eq!(decrypt(&key, &ciphertext).unwrap(), b"hello, vault");
    }

    #[test]
    fn decrypt_fails_with_the_wrong_key() {
        let key = random_vault_key();
        let other_key = random_vault_key();
        let ciphertext = encrypt(&key, b"secret").unwrap();
        assert!(decrypt(&other_key, &ciphertext).is_err());
    }

    #[test]
    fn decrypt_fails_on_tampered_ciphertext() {
        let key = random_vault_key();
        let mut ciphertext = encrypt(&key, b"secret").unwrap();
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xFF;
        assert!(decrypt(&key, &ciphertext).is_err());
    }

    #[test]
    fn two_encryptions_of_the_same_plaintext_differ() {
        // Different random nonces each time - a necessary property, since
        // a repeated nonce under the same key breaks XChaCha20Poly1305.
        let key = random_vault_key();
        let a = encrypt(&key, b"same text").unwrap();
        let b = encrypt(&key, b"same text").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn password_derived_key_is_deterministic_for_the_same_salt() {
        let salt = random_salt();
        let a = derive_key_from_password("correct horse battery staple", &salt).unwrap();
        let b = derive_key_from_password("correct horse battery staple", &salt).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn password_derived_key_differs_for_a_different_password_or_salt() {
        let salt = random_salt();
        let base = derive_key_from_password("password one", &salt).unwrap();
        assert_ne!(base, derive_key_from_password("password two", &salt).unwrap());
        assert_ne!(base, derive_key_from_password("password one", &random_salt()).unwrap());
    }

    #[test]
    fn recovery_key_round_trips_through_its_text_format() {
        let key = random_recovery_key();
        let formatted = format_recovery_key(&key);
        // 16 groups of 4 hex chars (2 bytes each), joined by 15 dashes.
        assert_eq!(formatted.len(), KEY_LEN * 2 + (KEY_LEN / 2 - 1));
        assert_eq!(parse_recovery_key(&formatted).unwrap(), key);
    }

    #[test]
    fn parse_recovery_key_is_case_insensitive_and_whitespace_tolerant() {
        let key = random_recovery_key();
        let formatted = format_recovery_key(&key);
        let messy = format!("  {} \n", formatted.to_lowercase());
        assert_eq!(parse_recovery_key(&messy).unwrap(), key);
    }

    #[test]
    fn parse_recovery_key_rejects_the_wrong_length_or_non_hex_input() {
        assert!(parse_recovery_key("too-short").is_err());
        assert!(parse_recovery_key(&"ZZ-".repeat(32)).is_err());
    }

    #[test]
    fn password_and_recovery_key_can_independently_unwrap_the_same_vault_key() {
        // This is the actual property the "lost password but have the
        // recovery key" flow depends on: both wrap *the same* VK.
        let vault_key = random_vault_key();
        let salt = random_salt();
        let password_key = derive_key_from_password("hunter2", &salt).unwrap();
        let recovery_key = random_recovery_key();

        let wrapped_by_password = encrypt(&password_key, &vault_key).unwrap();
        let wrapped_by_recovery = encrypt(&recovery_key, &vault_key).unwrap();

        let unwrapped_a = decrypt(&derive_key_from_password("hunter2", &salt).unwrap(), &wrapped_by_password).unwrap();
        let unwrapped_b = decrypt(&recovery_key, &wrapped_by_recovery).unwrap();
        assert_eq!(unwrapped_a, vault_key);
        assert_eq!(unwrapped_b, vault_key);
    }
}
