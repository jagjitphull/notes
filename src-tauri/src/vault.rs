//! Whether the current notes folder's content is encrypted at rest, and the
//! key material needed to read/write it.
//!
//! The vault's "keyring" is a small, non-secret file holding a salt plus
//! two *wrapped* copies of a random Vault Key (VK), one under a key derived
//! from the user's password and one under a separately-generated recovery
//! key. It lives inside notes_root itself (`.vault-keyring.json`, which
//! `store::full_rescan`'s dot-prefix filter already ignores) specifically
//! so it travels with the vault through whatever sync tool already moves
//! the notes themselves - the same password unlocks the vault from any
//! machine that has synced it. The VK itself is never written to disk,
//! only these two wrapped copies are, and it's held in memory only for
//! the current app session (see [`VaultState`]), cleared on lock or exit.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::vault_crypto::{self, KEY_LEN, SALT_LEN};

pub type VaultResult<T> = Result<T, String>;

const KEYRING_FILENAME: &str = ".vault-keyring.json";

/// Which form a note file on disk is in, and (if encrypted) the key to
/// read/write it. Threaded explicitly through every store.rs function that
/// touches note file content, rather than read from ambient/global state -
/// this is the one place a bug could mean silently writing plaintext to
/// disk for a vault the user believes is encrypted, so it's worth the
/// larger diff to have the compiler check every call site supplies one.
#[derive(Clone, Copy)]
pub enum NoteCodec {
    Plain,
    Encrypted([u8; KEY_LEN]),
}

impl NoteCodec {
    pub fn is_encrypted(&self) -> bool {
        matches!(self, Self::Encrypted(_))
    }

    pub fn extension(&self) -> &'static str {
        if self.is_encrypted() { "menc" } else { "md" }
    }

    /// Reads a note file's content, decrypting it first if this codec is
    /// encrypted. `fs::read` (not `read_to_string`) since encrypted content
    /// is binary, not valid UTF-8, until decrypted.
    pub fn read(&self, path: &Path) -> VaultResult<String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let plain = match self {
            Self::Plain => bytes,
            Self::Encrypted(key) => vault_crypto::decrypt(key, &bytes)?,
        };
        String::from_utf8(plain).map_err(|e| e.to_string())
    }

    pub fn write(&self, path: &Path, content: &str) -> VaultResult<()> {
        let bytes = match self {
            Self::Plain => content.as_bytes().to_vec(),
            Self::Encrypted(key) => vault_crypto::encrypt(key, content.as_bytes())?,
        };
        fs::write(path, bytes).map_err(|e| e.to_string())
    }
}

/// The current session's unlocked vault key, if any - Tauri-managed state,
/// one per running app. `None` whether the active vault isn't encrypted at
/// all, or is encrypted but still locked; callers distinguish those with
/// [`is_encrypted`].
pub struct VaultState(pub Mutex<Option<[u8; KEY_LEN]>>);

impl VaultState {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

impl Default for VaultState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct Keyring {
    salt: String,
    password_wrapped: String,
    recovery_wrapped: String,
}

fn keyring_path(notes_root: &Path) -> PathBuf {
    notes_root.join(KEYRING_FILENAME)
}

/// Whether `notes_root` is an encrypted vault - independent of whether it's
/// currently unlocked.
pub fn is_encrypted(notes_root: &Path) -> bool {
    keyring_path(notes_root).exists()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> VaultResult<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err("corrupted keyring: odd-length hex".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn fixed_bytes<const N: usize>(bytes: Vec<u8>, what: &str) -> VaultResult<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| format!("corrupted keyring: wrong {what} length"))
}

fn load_keyring(notes_root: &Path) -> VaultResult<Keyring> {
    let raw = fs::read_to_string(keyring_path(notes_root)).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

/// Turns on encryption for a vault that isn't encrypted yet: generates a
/// fresh Vault Key and a fresh recovery key, wraps the VK under both the
/// password and the recovery key, and writes the keyring. Returns the raw
/// VK (so the caller can migrate existing notes and unlock the session in
/// the same step) and the recovery key formatted for one-time display -
/// this is the only moment it's ever recoverable; it isn't stored anywhere.
pub fn enable(notes_root: &Path, password: &str) -> VaultResult<([u8; KEY_LEN], String)> {
    if is_encrypted(notes_root) {
        return Err("this vault is already encrypted".to_string());
    }
    if password.is_empty() {
        return Err("password can't be empty".to_string());
    }

    let vault_key = vault_crypto::random_vault_key();
    let salt = vault_crypto::random_salt();
    let password_key = vault_crypto::derive_key_from_password(password, &salt)?;
    let recovery_key = vault_crypto::random_recovery_key();

    let keyring = Keyring {
        salt: hex_encode(&salt),
        password_wrapped: hex_encode(&vault_crypto::encrypt(&password_key, &vault_key)?),
        recovery_wrapped: hex_encode(&vault_crypto::encrypt(&recovery_key, &vault_key)?),
    };
    let json = serde_json::to_string_pretty(&keyring).map_err(|e| e.to_string())?;
    fs::write(keyring_path(notes_root), json).map_err(|e| e.to_string())?;

    Ok((vault_key, vault_crypto::format_recovery_key(&recovery_key)))
}

pub fn unlock_with_password(notes_root: &Path, password: &str) -> VaultResult<[u8; KEY_LEN]> {
    let keyring = load_keyring(notes_root)?;
    let salt: [u8; SALT_LEN] = fixed_bytes(hex_decode(&keyring.salt)?, "salt")?;
    let password_key = vault_crypto::derive_key_from_password(password, &salt)?;
    let wrapped = hex_decode(&keyring.password_wrapped)?;
    let vault_key = vault_crypto::decrypt(&password_key, &wrapped).map_err(|_| "incorrect password".to_string())?;
    fixed_bytes(vault_key, "vault key")
}

pub fn unlock_with_recovery_key(notes_root: &Path, recovery_key_input: &str) -> VaultResult<[u8; KEY_LEN]> {
    let recovery_key = vault_crypto::parse_recovery_key(recovery_key_input)?;
    let keyring = load_keyring(notes_root)?;
    let wrapped = hex_decode(&keyring.recovery_wrapped)?;
    let vault_key =
        vault_crypto::decrypt(&recovery_key, &wrapped).map_err(|_| "incorrect recovery key".to_string())?;
    fixed_bytes(vault_key, "vault key")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("notes-vault-test-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_fresh_notes_root_is_not_encrypted() {
        let dir = temp_dir("fresh");
        assert!(!is_encrypted(&dir));
    }

    #[test]
    fn enable_then_unlock_with_password_recovers_the_same_key() {
        let dir = temp_dir("enable-unlock");
        let (vault_key, _recovery) = enable(&dir, "hunter2").unwrap();
        assert!(is_encrypted(&dir));

        let unlocked = unlock_with_password(&dir, "hunter2").unwrap();
        assert_eq!(unlocked, vault_key);
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let dir = temp_dir("wrong-password");
        enable(&dir, "hunter2").unwrap();
        assert!(unlock_with_password(&dir, "not it").is_err());
    }

    #[test]
    fn unlock_with_the_recovery_key_recovers_the_same_key_as_the_password() {
        let dir = temp_dir("recovery-unlock");
        let (vault_key, recovery) = enable(&dir, "hunter2").unwrap();

        let unlocked = unlock_with_recovery_key(&dir, &recovery).unwrap();
        assert_eq!(unlocked, vault_key);
    }

    #[test]
    fn unlock_with_the_wrong_recovery_key_fails() {
        let dir = temp_dir("wrong-recovery");
        enable(&dir, "hunter2").unwrap();
        let other_recovery = vault_crypto::format_recovery_key(&vault_crypto::random_recovery_key());
        assert!(unlock_with_recovery_key(&dir, &other_recovery).is_err());
    }

    #[test]
    fn enabling_an_already_encrypted_vault_is_refused() {
        let dir = temp_dir("double-enable");
        enable(&dir, "hunter2").unwrap();
        assert!(enable(&dir, "another password").is_err());
    }

    #[test]
    fn note_codec_round_trips_plain_and_encrypted_content() {
        let dir = temp_dir("codec-round-trip");

        let plain_path = dir.join("note.md");
        NoteCodec::Plain.write(&plain_path, "hello plain").unwrap();
        assert_eq!(NoteCodec::Plain.read(&plain_path).unwrap(), "hello plain");

        let key = vault_crypto::random_vault_key();
        let encrypted_path = dir.join("note.menc");
        let codec = NoteCodec::Encrypted(key);
        codec.write(&encrypted_path, "hello secret").unwrap();
        assert_eq!(codec.read(&encrypted_path).unwrap(), "hello secret");

        // The bytes actually on disk must not contain the plaintext.
        let raw = fs::read(&encrypted_path).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"secret"));
    }
}
