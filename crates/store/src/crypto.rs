//! Encryption with age and a passphrase (scrypt). age is an audited, widely used format; its
//! authenticated encryption also detects any change to the encrypted content.

use age::secrecy::SecretString;

use crate::StoreError;

/// Lowest and highest scrypt work factor this program writes (2^n iterations).
pub const WORK_FACTOR_RANGE: std::ops::RangeInclusive<u8> = 10..=22;

/// Encrypts `plain` with a passphrase. `work_factor` `None` lets age pick about one second of
/// work on this device.
pub fn encrypt(
    plain: &[u8],
    passphrase: &SecretString,
    work_factor: Option<u8>,
) -> Result<Vec<u8>, StoreError> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    if let Some(factor) = work_factor {
        let factor = factor.clamp(*WORK_FACTOR_RANGE.start(), *WORK_FACTOR_RANGE.end());
        recipient.set_work_factor(factor);
    }
    age::encrypt(&recipient, plain).map_err(|e| StoreError::Encryption {
        reason: e.to_string(),
    })
}

/// Decrypts with a passphrase.
pub fn decrypt(cipher: &[u8], passphrase: &SecretString) -> Result<Vec<u8>, StoreError> {
    let mut identity = age::scrypt::Identity::new(passphrase.clone());
    identity.set_max_work_factor(*WORK_FACTOR_RANGE.end());
    age::decrypt(&identity, cipher).map_err(|error| match error {
        age::DecryptError::DecryptionFailed
        | age::DecryptError::NoMatchingKeys
        | age::DecryptError::KeyDecryptionFailed => StoreError::WrongPassphrase,
        age::DecryptError::ExcessiveWork { required, .. } => StoreError::Encryption {
            reason: format!("the file demands too much work (scrypt factor {required})"),
        },
        other => StoreError::corrupt(format!("encrypted content: {other}")),
    })
}
