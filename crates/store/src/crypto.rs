//! Encryption with age and a passphrase (scrypt). age is an audited, widely used format; its
//! authenticated encryption also detects any change to the encrypted content.
//!
//! The scrypt work factor is fixed, not measured on the writing machine: a fast desktop would
//! otherwise write files the browser cannot open, and a debug build files that are weakly
//! protected. Reading caps the factor so a hostile file cannot demand gigabytes of memory.

use age::secrecy::{ExposeSecret, SecretString};
use unicode_normalization::UnicodeNormalization;

/// Shortest passphrase accepted when a file is protected, counted after Unicode normalisation
/// (as it is used).
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// Whether a new passphrase is long enough.
pub fn passphrase_long_enough(passphrase: &str) -> bool {
    passphrase.nfc().count() >= MIN_PASSPHRASE_CHARS
}

use crate::StoreError;

/// The scrypt work factor every file is written with: 2^18 rounds, 256 MiB of memory per guess.
pub const WORK_FACTOR: u8 = 18;

/// Lowest and highest factor this program writes (lower only for tests).
pub const WORK_FACTOR_RANGE: std::ops::RangeInclusive<u8> = 10..=WORK_FACTOR;

/// Highest factor accepted when reading. The browser has less memory to spare.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_READ_WORK_FACTOR: u8 = 20;
/// Highest factor accepted when reading. The browser has less memory to spare.
#[cfg(target_arch = "wasm32")]
pub const MAX_READ_WORK_FACTOR: u8 = WORK_FACTOR;

// Own files must always open, and a hostile file may cost at most about 1 GiB of memory.
const _: () = assert!(WORK_FACTOR <= MAX_READ_WORK_FACTOR && MAX_READ_WORK_FACTOR <= 20);

/// Plaintext is padded to a multiple of this many bytes, so the file size reveals little about
/// the content.
const PADDING: usize = 4096;

/// The same passphrase typed on different systems can arrive as different Unicode sequences
/// ("ä" as one or two code points); normalising makes them equal.
fn normalise(passphrase: &SecretString) -> SecretString {
    SecretString::from(passphrase.expose_secret().nfc().collect::<String>())
}

fn pad(plain: &[u8]) -> Vec<u8> {
    let length = u64::try_from(plain.len()).unwrap_or(u64::MAX);
    let mut out = Vec::with_capacity(plain.len() + 8 + PADDING);
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(plain);
    let rest = out.len() % PADDING;
    if rest != 0 {
        out.resize(out.len() + (PADDING - rest), 0);
    }
    out
}

fn unpad(padded: &[u8]) -> Result<&[u8], StoreError> {
    let corrupt = || StoreError::corrupt("the decrypted content has a wrong length");
    let (head, rest) = padded.split_at_checked(8).ok_or_else(corrupt)?;
    let mut length = [0u8; 8];
    length.copy_from_slice(head);
    let length = usize::try_from(u64::from_le_bytes(length)).map_err(|_| corrupt())?;
    let (content, padding) = rest.split_at_checked(length).ok_or_else(corrupt)?;
    if padding.len() >= PADDING || padding.iter().any(|b| *b != 0) {
        return Err(corrupt());
    }
    Ok(content)
}

/// The factor actually written: [`WORK_FACTOR`] unless a lower one is asked for, never outside
/// [`WORK_FACTOR_RANGE`] (age itself would panic at 0 or 64).
fn factor(requested: Option<u8>) -> u8 {
    requested
        .unwrap_or(WORK_FACTOR)
        .clamp(*WORK_FACTOR_RANGE.start(), *WORK_FACTOR_RANGE.end())
}

/// Encrypts `plain` with a passphrase at [`WORK_FACTOR`] (or a lower factor for tests).
pub fn encrypt(
    plain: &[u8],
    passphrase: &SecretString,
    work_factor: Option<u8>,
) -> Result<Vec<u8>, StoreError> {
    let mut recipient = age::scrypt::Recipient::new(normalise(passphrase));
    recipient.set_work_factor(factor(work_factor));
    age::encrypt(&recipient, &pad(plain)).map_err(|e| StoreError::Encryption {
        reason: e.to_string(),
    })
}

/// Decrypts with a passphrase, accepting work factors up to [`MAX_READ_WORK_FACTOR`].
pub fn decrypt(cipher: &[u8], passphrase: &SecretString) -> Result<Vec<u8>, StoreError> {
    decrypt_capped(cipher, passphrase, MAX_READ_WORK_FACTOR)
}

pub(crate) fn decrypt_capped(
    cipher: &[u8],
    passphrase: &SecretString,
    cap: u8,
) -> Result<Vec<u8>, StoreError> {
    let mut identity = age::scrypt::Identity::new(normalise(passphrase));
    identity.set_max_work_factor(cap);
    let padded = age::decrypt(&identity, cipher).map_err(|error| match error {
        age::DecryptError::DecryptionFailed
        | age::DecryptError::NoMatchingKeys
        | age::DecryptError::KeyDecryptionFailed => StoreError::WrongPassphrase,
        age::DecryptError::ExcessiveWork { required, .. } => StoreError::Encryption {
            reason: format!("the file demands too much work (scrypt factor {required})"),
        },
        other => StoreError::corrupt(format!("encrypted content: {other}")),
    })?;
    unpad(&padded).map(<[u8]>::to_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    #[test]
    fn the_read_cap_refuses_expensive_files() {
        let cipher = encrypt(b"x", &pass("p"), Some(12)).unwrap();
        assert!(matches!(
            decrypt_capped(&cipher, &pass("p"), 11),
            Err(StoreError::Encryption { .. })
        ));
        assert_eq!(decrypt_capped(&cipher, &pass("p"), 12).unwrap(), b"x");
    }

    #[test]
    fn caps_and_defaults_fit_together() {
        // Absurd requests are clamped, never passed to age (which would panic at 0 or 64).
        assert_eq!(factor(None), WORK_FACTOR);
        assert_eq!(factor(Some(255)), WORK_FACTOR);
        assert_eq!(factor(Some(0)), 10);
        assert_eq!(factor(Some(12)), 12);
    }

    #[test]
    fn passphrases_are_compared_after_unicode_normalisation() {
        let composed = pass("Übung\u{00E4}");
        let decomposed = pass("U\u{0308}bunga\u{0308}");
        let cipher = encrypt(b"z", &composed, Some(10)).unwrap();
        assert_eq!(decrypt(&cipher, &decomposed).unwrap(), b"z");
    }

    #[test]
    fn padding_hides_the_exact_size() {
        let small = encrypt(&[1u8; 10], &pass("p"), Some(10)).unwrap();
        let larger = encrypt(&[1u8; 3000], &pass("p"), Some(10)).unwrap();
        assert_eq!(small.len(), larger.len());
    }

    #[test]
    fn wrong_padding_is_refused() {
        let mut padded = pad(b"abc");
        assert_eq!(unpad(&padded).unwrap(), b"abc");
        if let Some(last) = padded.last_mut() {
            *last = 1;
        }
        assert!(unpad(&padded).is_err());
        assert!(unpad(&[1, 2, 3]).is_err());
        let mut overpadded = pad(b"abc");
        overpadded.extend(std::iter::repeat_n(0u8, PADDING));
        assert!(
            unpad(&overpadded).is_err(),
            "padding of a whole block or more is refused"
        );
        let mut too_long = 9_999u64.to_le_bytes().to_vec();
        too_long.extend_from_slice(b"short");
        assert!(unpad(&too_long).is_err());
    }
}
