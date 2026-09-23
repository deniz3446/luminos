use std::{
    error::Error,
    fmt,
};

use ring::signature::{
    ED25519,
    UnparsedPublicKey,
};

pub const PINNED_UPDATE_PUBLIC_KEY_SPKI_SHA256: &str =
    "1825e4096174f21e0413f2bf4acc6328f944a31040a0bbd5ee6c5bf415013d71";

const PINNED_UPDATE_PUBLIC_KEY_SPKI: [u8; 44] = [
    0x30, 0x2a, 0x30, 0x05,
    0x06, 0x03, 0x2b, 0x65,
    0x70, 0x03, 0x21, 0x00,
    0xda, 0x85, 0x81, 0xba,
    0xcd, 0x43, 0x80, 0x51,
    0xfc, 0xe1, 0x68, 0x3a,
    0x67, 0xb4, 0x6a, 0x1a,
    0x26, 0xc1, 0x80, 0x62,
    0xea, 0xd9, 0x07, 0xdc,
    0xff, 0x2f, 0x34, 0xa7,
    0xfe, 0x93, 0x6e, 0x97,
];

const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05,
    0x06, 0x03, 0x2b, 0x65,
    0x70, 0x03, 0x21, 0x00,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTrustError {
    InvalidPinnedKey,
    InvalidSignature,
}

impl fmt::Display for UpdateTrustError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::InvalidPinnedKey => {
                write!(
                    f,
                    "PhotoOS pinned update public key is invalid"
                )
            }

            Self::InvalidSignature => {
                write!(
                    f,
                    "PhotoOS update signature verification failed"
                )
            }
        }
    }
}

impl Error for UpdateTrustError {}

pub fn pinned_public_key_spki() -> &'static [u8] {
    &PINNED_UPDATE_PUBLIC_KEY_SPKI
}

fn pinned_public_key_raw(
) -> Result<&'static [u8], UpdateTrustError> {
    let spki = pinned_public_key_spki();

    if spki.len() != 44 {
        return Err(UpdateTrustError::InvalidPinnedKey);
    }

    if spki[..ED25519_SPKI_PREFIX.len()]
        != ED25519_SPKI_PREFIX
    {
        return Err(UpdateTrustError::InvalidPinnedKey);
    }

    let raw = &spki[ED25519_SPKI_PREFIX.len()..];

    if raw.len() != 32 {
        return Err(UpdateTrustError::InvalidPinnedKey);
    }

    Ok(raw)
}

pub fn verify_detached_signature(
    payload: &[u8],
    signature: &[u8],
) -> Result<(), UpdateTrustError> {
    if signature.len() != 64 {
        return Err(UpdateTrustError::InvalidSignature);
    }

    let public_key = pinned_public_key_raw()?;

    UnparsedPublicKey::new(
        &ED25519,
        public_key,
    )
    .verify(
        payload,
        signature,
    )
    .map_err(
        |_| UpdateTrustError::InvalidSignature,
    )
}


#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{
        pinned_public_key_spki,
        verify_detached_signature,
        PINNED_UPDATE_PUBLIC_KEY_SPKI_SHA256,
    };

    const EXPECTED_SPKI_SHA256: &str =
        "1825e4096174f21e0413f2bf4acc6328f944a31040a0bbd5ee6c5bf415013d71";

    const PRODUCTION_SPKI: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/photoos-update-ed25519.spki.der"
    );

    const BETA_MANIFEST: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json"
    );

    const BETA_SIGNATURE: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json.sig"
    );

    fn hex_lower(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);

        for byte in bytes {
            use std::fmt::Write;
            write!(&mut out, "{byte:02x}").unwrap();
        }

        out
    }

    #[test]
    fn pinned_public_key_matches_production_spki_identity() {
        assert_eq!(
            PINNED_UPDATE_PUBLIC_KEY_SPKI_SHA256,
            EXPECTED_SPKI_SHA256
        );

        assert_eq!(
            pinned_public_key_spki(),
            PRODUCTION_SPKI
        );

        let digest = Sha256::digest(
            pinned_public_key_spki()
        );

        assert_eq!(
            hex_lower(&digest),
            EXPECTED_SPKI_SHA256
        );
    }

    #[test]
    fn production_beta_1_2_3_signature_verifies() {
        verify_detached_signature(
            BETA_MANIFEST,
            BETA_SIGNATURE,
        )
        .expect(
            "published PhotoOS 1.2.3 beta manifest must verify"
        );
    }

    #[test]
    fn tampered_manifest_is_rejected() {
        let mut tampered = BETA_MANIFEST.to_vec();

        assert!(!tampered.is_empty());

        tampered[0] ^= 0x01;

        assert!(
            verify_detached_signature(
                &tampered,
                BETA_SIGNATURE,
            )
            .is_err()
        );
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let mut tampered = BETA_SIGNATURE.to_vec();

        assert_eq!(tampered.len(), 64);

        tampered[0] ^= 0x01;

        assert!(
            verify_detached_signature(
                BETA_MANIFEST,
                &tampered,
            )
            .is_err()
        );
    }
}
