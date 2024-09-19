// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright by contributors to this project.
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use nss_gk_api::{PrivateKey, PublicKey};

use alloc::vec::Vec;
use mls_rs_crypto_traits::Curve;

#[cfg(feature = "std")]
use std::array::TryFromSliceError;

#[cfg(not(feature = "std"))]
use core::array::TryFromSliceError;
use core::fmt::{self, Debug};

use crate::Hash;

#[derive(Debug, Clone)]
pub enum EcPublicKey {
    X25519(nss_gk_api::PublicKey),
    Ed25519(nss_gk_api::PublicKey),
    P256(nss_gk_api::PublicKey),
}

#[derive(Clone)]
pub enum EcPrivateKey {
    X25519(nss_gk_api::PrivateKey),
    Ed25519(nss_gk_api::PrivateKey),
    P256(nss_gk_api::PrivateKey),
}

#[derive(Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum EcError {
    #[cfg_attr(feature = "std", error("unsupported curve type"))]
    UnsupportedCurve,
    #[cfg_attr(feature = "std", error("invalid public key data"))]
    EcKeyInvalidKeyData,
    #[cfg_attr(feature = "std", error("ec key is not a signature key"))]
    EcKeyNotSignature,
    #[cfg_attr(feature = "std", error(transparent))]
    TryFromSliceError(TryFromSliceError),
    #[cfg_attr(feature = "std", error("rand error: {0:?}"))]
    RandCoreError(rand_core::Error),
    #[cfg_attr(feature = "std", error("ecdh key type mismatch"))]
    EcdhKeyTypeMismatch,
    #[cfg_attr(feature = "std", error("ec key is not an ecdh key"))]
    EcKeyNotEcdh,
    #[cfg_attr(feature = "std", error("general nss failure"))]
    GeneralFailure,
}

impl From<rand_core::Error> for EcError {
    fn from(value: rand_core::Error) -> Self {
        EcError::RandCoreError(value)
    }
}

impl From<TryFromSliceError> for EcError {
    fn from(e: TryFromSliceError) -> Self {
        EcError::TryFromSliceError(e)
    }
}

impl core::fmt::Debug for EcPrivateKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::X25519(_) => f.write_str("X25519 Secret Key"),
            Self::Ed25519(_) => f.write_str("Ed25519 Secret Key"),
            Self::P256(_) => f.write_str("P256 Secret Key"),
        }
    }
}

fn private_key_len(curve: Curve) -> usize {
    match curve {
        Curve::P256 => 0x20 as usize,
        Curve::Ed25519 | Curve::X25519 => 0x20 as usize,
        _ => 0 as usize,
    }
}

fn public_key_len(curve: Curve) -> usize {
    match curve {
        Curve::P256 => 0x41 as usize,
        Curve::Ed25519 | Curve::X25519 => 32 as usize,
        _ => 0 as usize,
    }
}

fn build_spki_from_raw_public_key(key: Vec<u8>, curve: Curve) -> Result<Vec<u8>, EcError> {
    let mut lh = {
        match curve {
            Curve::P256 => vec![
                0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06,
                0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
            ],
            Curve::Ed25519 => vec![
                0x30, 0x2a, 0x30, 0x5, 0x6, 0x3, 0x2b, 0x65, 0x70, 0x3, 0x21, 0x0,
            ],
            Curve::X25519 => vec![
                0x30, 0x2a, 0x30, 0x5, 0x6, 0x3, 0x2b, 0x65, 0x6e, 0x3, 0x21, 0x0,
            ],
            _ => return Err(EcError::UnsupportedCurve),
        }
    };

    let mut key = key.clone();

    if public_key_len(curve) == 0 {
        return Err(EcError::EcKeyInvalidKeyData);
    }

    // if the key length is bigger than the expected -> wrong key
    if key.len() > public_key_len(curve) {
        return Err(EcError::EcKeyInvalidKeyData);
    }

    // if the key length is shorted than expected -> extend the key
    if key.len() < public_key_len(curve) {
        let mut zeros = vec![0 as u8; public_key_len(curve) - key.len()];
        zeros.append(&mut key);
        lh.append(&mut zeros);
    } else {
        lh.append(&mut key);
    }

    Ok(lh)
}

pub fn pub_key_from_uncompressed(bytes: Vec<u8>, curve: Curve) -> Result<EcPublicKey, EcError> {
    let z = build_spki_from_raw_public_key(bytes, curve).unwrap();
    match curve {
        Curve::P256 => match nss_gk_api::ec::import_ec_public_key_from_spki(&z) {
            Ok(key) => return Ok(EcPublicKey::P256(key)),
            Err(_) => return Err(EcError::EcKeyInvalidKeyData),
        },
        Curve::Ed25519 => match nss_gk_api::ec::import_ec_public_key_from_spki(&z) {
            Ok(key) => return Ok(EcPublicKey::Ed25519(key)),
            Err(_) => return Err(EcError::EcKeyInvalidKeyData),
        },
        Curve::X25519 => match nss_gk_api::ec::import_ec_public_key_from_spki(&z) {
            Ok(key) => return Ok(EcPublicKey::X25519(key)),
            Err(_) => return Err(EcError::EcKeyInvalidKeyData),
        },
        _ => Err(EcError::UnsupportedCurve),
    }
}

pub fn pub_key_to_uncompressed(key: EcPublicKey) -> Result<Vec<u8>, EcError> {
    match key {
        EcPublicKey::P256(key) | EcPublicKey::Ed25519(key) | EcPublicKey::X25519(key) => {
            let k0 = key.key_data().unwrap();
            Ok(k0)
        }
    }
}

pub fn generate_private_key(curve: Curve) -> Result<EcPrivateKey, EcError> {
    let key_pair = nss_gk_api::ec::ecdh_keygen(nss_gk_api::ec::EcCurve::P256).unwrap();
    match curve {
        Curve::P256 => return Ok(EcPrivateKey::P256(key_pair.private)),
        Curve::X25519 => return Ok(EcPrivateKey::X25519(key_pair.private)),
        Curve::Ed25519 => return Ok(EcPrivateKey::Ed25519(key_pair.private)),
        _ => Err(EcError::UnsupportedCurve),
    }
}

pub fn private_key_from_pkcs8(bytes: &[u8], curve: Curve) -> Result<EcPrivateKey, EcError> {
    let private_key = nss_gk_api::ec::import_ec_private_key_pkcs8(bytes).unwrap();
    match curve {
        Curve::P256 => return Ok(EcPrivateKey::P256(private_key)),
        Curve::Ed25519 => return Ok(EcPrivateKey::Ed25519(private_key)),
        Curve::X25519 => return Ok(EcPrivateKey::X25519(private_key)),
        _ => Err(EcError::UnsupportedCurve),
    }
}

pub fn private_key_to_pkcs8(key: EcPrivateKey) -> Result<Vec<u8>, EcError> {
    match key {
        EcPrivateKey::P256(key) | EcPrivateKey::Ed25519(key) | EcPrivateKey::X25519(key) => {
            match nss_gk_api::ec::export_ec_private_key_pkcs8(key) {
                Ok(key) => return Ok(key),
                Err(_) => return Err(EcError::EcKeyInvalidKeyData),
            }
        }
        _ => Err(EcError::UnsupportedCurve),
    }
}

fn build_pkcs8_from_raw_private_key(key: Vec<u8>, curve: Curve) -> Result<Vec<u8>, EcError> {
    let mut lh = {
        match curve {
            Curve::P256 => vec![
                0x30, 0x41, 0x2, 0x1, 0x0, 0x30, 0x13, 0x6, 0x7, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x2,
                0x1, 0x6, 0x8, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x3, 0x1, 0x7, 0x4, 0x27, 0x30, 0x25,
                0x2, 0x1, 0x1, 0x4, 0x20,
            ],
            Curve::Ed25519 => vec![
                0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
                0x04, 0x20,
            ],
            Curve::X25519 => vec![
                0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22,
                0x04, 0x20,
            ],
            _ => return Err(EcError::UnsupportedCurve),
        }
    };

    let mut key = key.clone();

    if private_key_len(curve) == 0 {
        return Err(EcError::EcKeyInvalidKeyData);
    }

    // if the key length is bigger than the expected -> wrong key
    if key.len() > private_key_len(curve) {
        return Err(EcError::EcKeyInvalidKeyData);
    }

    // if the key length is shorted than expected -> extend the key
    if key.len() < private_key_len(curve) {
        let mut zeros = vec![0 as u8; private_key_len(curve) - key.len()];
        zeros.append(&mut key);
        lh.append(&mut zeros);
    } else {
        lh.append(&mut key);
    }

    Ok(lh)
}

fn private_key_from_bytes_helper(bytes: Vec<u8>, curve: Curve) -> Vec<u8> {
    if (is_secret_key_contains_public_key(bytes.clone(), curve)) {
        let (test, _) = bytes.split_at(private_key_len(curve));
        return test.to_vec();
    }
    return bytes;
}

pub fn private_key_from_bytes(bytes: Vec<u8>, curve: Curve) -> Result<EcPrivateKey, EcError> {
    let private_key = private_key_from_bytes_helper(bytes, curve);
    let private_key_pkcs8 = build_pkcs8_from_raw_private_key(private_key, curve).unwrap();
    let private_key_imported =
        nss_gk_api::ec::import_ec_private_key_pkcs8(&private_key_pkcs8).unwrap();
    match curve {
        Curve::P256 => Ok(EcPrivateKey::P256(private_key_imported)),
        Curve::Ed25519 => Ok(EcPrivateKey::Ed25519(private_key_imported)),
        Curve::X25519 => Ok(EcPrivateKey::X25519(private_key_imported)),
        _ => Err(EcError::UnsupportedCurve),
    }
}

pub fn private_key_to_bytes(key: EcPrivateKey) -> Result<Vec<u8>, EcError> {
    match key {
        EcPrivateKey::Ed25519(key) | EcPrivateKey::P256(key) | EcPrivateKey::X25519(key) => {
            let key = nss_gk_api::ec::export_ec_private_key_from_raw(key).unwrap();
            Ok(key)
        }
    }
}

pub fn private_key_to_public(private_key: &EcPrivateKey) -> Result<EcPublicKey, EcError> {
    match private_key {
        EcPrivateKey::X25519(key) => Ok(EcPublicKey::X25519(
            nss_gk_api::ec::convert_to_public(key.clone()).unwrap(),
        )),
        EcPrivateKey::Ed25519(key) => Ok(EcPublicKey::Ed25519(
            nss_gk_api::ec::convert_to_public(key.clone()).unwrap(),
        )),
        EcPrivateKey::P256(key) => Ok(EcPublicKey::P256(
            nss_gk_api::ec::convert_to_public(key.clone()).unwrap(),
        )),
    }
}

pub fn private_key_ecdh(
    private_key: &EcPrivateKey,
    remote_public: &EcPublicKey,
) -> Result<Vec<u8>, EcError> {
    let shared_secret = match private_key {
        EcPrivateKey::X25519(private_key) => match remote_public {
            EcPublicKey::X25519(public) => {
                let r = nss_gk_api::ec::ecdh(private_key.clone(), public.clone()).unwrap();
                Ok(r)
            }
            _ => Err(EcError::EcdhKeyTypeMismatch),
        },
        EcPrivateKey::Ed25519(_) => Err(EcError::EcKeyNotEcdh),
        EcPrivateKey::P256(private_key) => match remote_public {
            EcPublicKey::P256(public) => {
                let r = nss_gk_api::ec::ecdh(private_key.clone(), public.clone()).unwrap();
                Ok(r)
            }
            _ => Err(EcError::EcdhKeyTypeMismatch),
        },
    }?;

    Ok(shared_secret)
}

pub fn sign_p256(private_key: PrivateKey, data: &[u8]) -> Result<Vec<u8>, EcError> {
    let mut hashed_data = Hash::hash(&Hash::Sha256, data);
    let signature = nss_gk_api::ec::sign_ecdsa(private_key, hashed_data.as_mut()).unwrap();
    Ok(signature)
}

pub fn verify_p256(public_key: PublicKey, signature: &[u8], data: &[u8]) -> Result<bool, EcError> {
    let mut hashed_data = Hash::hash(&Hash::Sha256, data);
    let result = nss_gk_api::ec::verify_ecdsa(public_key, hashed_data.as_mut(), signature).unwrap();
    Ok(result)
}

pub fn sign_ed25519(private_key: PrivateKey, data: &[u8]) -> Result<Vec<u8>, EcError> {
    let signature = nss_gk_api::ec::sign_eddsa(private_key, &data).unwrap();
    Ok(signature)
}

pub fn verify_ed25519(
    public_key: PublicKey,
    signature: &[u8],
    data: &[u8],
) -> Result<bool, EcError> {
    let result = nss_gk_api::ec::verify_eddsa(public_key, &data, signature).unwrap();
    Ok(result)
}

pub fn generate_keypair(curve: Curve) -> Result<KeyPair, EcError> {
    match curve {
        Curve::P256 => {
            let key = nss_gk_api::ec::ecdh_keygen(nss_gk_api::ec::EcCurve::P256).unwrap();
            let secret: Vec<u8> = private_key_to_bytes(EcPrivateKey::P256(key.private))?;
            let public: Vec<u8> = pub_key_to_uncompressed(EcPublicKey::P256(key.public))?;
            return Ok(KeyPair { public, secret });
        }
        Curve::Ed25519 => {
            let key = nss_gk_api::ec::ecdh_keygen(nss_gk_api::ec::EcCurve::Ed25519).unwrap();
            let secret: Vec<u8> = private_key_to_bytes(EcPrivateKey::Ed25519(key.private))?;
            let public: Vec<u8> = pub_key_to_uncompressed(EcPublicKey::Ed25519(key.public))?;
            return Ok(KeyPair { public, secret });
        }
        Curve::X25519 => {
            let key = nss_gk_api::ec::ecdh_keygen(nss_gk_api::ec::EcCurve::X25519).unwrap();
            let secret: Vec<u8> = private_key_to_bytes(EcPrivateKey::X25519(key.private))?;
            let public: Vec<u8> = pub_key_to_uncompressed(EcPublicKey::X25519(key.public))?;
            return Ok(KeyPair { public, secret });
        }
        _ => {
            let secret = generate_private_key(curve)?;
            let public = private_key_to_public(&secret)?;
            let secret = private_key_to_bytes(secret)?;
            let public = pub_key_to_uncompressed(public)?;
            Ok(KeyPair { public, secret })
        }
    }
}

#[derive(Clone, Default)]
pub struct KeyPair {
    pub public: Vec<u8>,
    pub secret: Vec<u8>,
}

impl Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("public", &mls_rs_core::debug::pretty_bytes(&self.public))
            .field("secret", &mls_rs_core::debug::pretty_bytes(&self.secret))
            .finish()
    }
}

fn is_secret_key_contains_public_key(secret_key: Vec<u8>, curve: Curve) -> bool {
    let private_key_len = private_key_len(curve);
    let public_key_len = public_key_len(curve);
    if (secret_key.len() == private_key_len + public_key_len) {
        return true;
    }
    return false;
}

pub fn private_key_bytes_to_public(secret_key: Vec<u8>, curve: Curve) -> Result<Vec<u8>, EcError> {
    if (!is_secret_key_contains_public_key(secret_key.clone(), curve)) {
        let secret_key = private_key_from_bytes(secret_key.clone(), curve)?;
        let public_key = private_key_to_public(&secret_key)?;
        pub_key_to_uncompressed(public_key)
    } else {
        let (_, public_key) = secret_key.split_at(private_key_len(curve));
        Ok(public_key.to_vec())
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use serde::Deserialize;

    use super::Curve;

    use alloc::vec::Vec;

    #[derive(Deserialize)]
    pub(crate) struct TestKeys {
        #[serde(with = "hex::serde")]
        p256: Vec<u8>,
        #[serde(with = "hex::serde")]
        x25519: Vec<u8>,
        #[serde(with = "hex::serde")]
        ed25519: Vec<u8>,
    }

    impl TestKeys {
        pub(crate) fn get_key_from_curve(&self, curve: Curve) -> Vec<u8> {
            match curve {
                Curve::P256 => self.p256.clone(),
                Curve::X25519 => self.x25519.clone(),
                Curve::Ed25519 => self.ed25519.clone(),
                _ => Vec::new(),
            }
        }
    }

    pub(crate) fn get_test_public_keys() -> TestKeys {
        let test_case_file = include_str!("../test_data/test_public_keys.json");
        serde_json::from_str(test_case_file).unwrap()
    }

    pub(crate) fn get_test_secret_keys() -> TestKeys {
        let test_case_file = include_str!("../test_data/test_private_keys.json");
        serde_json::from_str(test_case_file).unwrap()
    }

    pub fn is_curve_25519(curve: Curve) -> bool {
        curve == Curve::X25519 || curve == Curve::Ed25519
    }

    pub fn byte_equal(curve: Curve, other: Curve) -> bool {
        if curve == other {
            return true;
        }

        if is_curve_25519(curve) && is_curve_25519(other) {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::{
        generate_keypair, generate_private_key, private_key_bytes_to_public,
        private_key_from_bytes, private_key_from_pkcs8, private_key_to_bytes, private_key_to_pkcs8,
        pub_key_from_uncompressed, pub_key_to_uncompressed, sign_ed25519, sign_p256,
        test_utils::{byte_equal, get_test_public_keys, get_test_secret_keys},
        verify_ed25519, verify_p256, Curve, EcError, EcPublicKey,
    };

    use alloc::vec;
    const SUPPORTED_CURVES: [Curve; 3] = [Curve::Ed25519, Curve::P256, Curve::X25519];

    #[test]
    fn private_key_can_be_generated() {
        SUPPORTED_CURVES.iter().copied().for_each(|curve| {
            let one_key = generate_private_key(curve)
                .unwrap_or_else(|e| panic!("Failed to generate private key for {curve:?} : {e:?}"));

            let another_key = generate_private_key(curve)
                .unwrap_or_else(|e| panic!("Failed to generate private key for {curve:?} : {e:?}"));

            assert_ne!(
                private_key_to_bytes(one_key).unwrap(),
                private_key_to_bytes(another_key).unwrap(),
                "Same key generated twice for {curve:?}"
            );
        });
    }

    #[test]
    fn key_pair_can_be_generated() {
        SUPPORTED_CURVES.iter().copied().for_each(|curve| {
            assert_matches!(
                generate_keypair(curve),
                Ok(_),
                "Failed to generate key pair for {curve:?}"
            );
        });
    }

    #[test]
    fn private_key_can_be_imported_and_exported() {
        SUPPORTED_CURVES.iter().copied().for_each(|curve| {
            let key_bytes = get_test_secret_keys().get_key_from_curve(curve);

            let imported_key = private_key_from_bytes(key_bytes.clone(), curve)
                .unwrap_or_else(|e| panic!("Failed to import private key for {curve:?} : {e:?}"));

            let exported_bytes = private_key_to_bytes(imported_key)
                .unwrap_or_else(|e| panic!("Failed to export private key for {curve:?} : {e:?}"));

            assert_eq!(exported_bytes, key_bytes);
        });
    }

    #[test]
    fn private_key_pkcs8_can_be_imported_and_exported() {
        SUPPORTED_CURVES.iter().copied().for_each(|curve| {
            let key = generate_private_key(curve)
                .unwrap_or_else(|e| panic!("Failed to generate private key for {curve:?} : {e:?}"));

            let exported_bytes = private_key_to_pkcs8(key)
                .unwrap_or_else(|e| panic!("Failed to export private key for {curve:?} : {e:?}"));

            let imported_key = private_key_from_pkcs8(&exported_bytes, curve)
                .unwrap_or_else(|e| panic!("Failed to import private key for {curve:?} : {e:?}"));

            let exported_bytes_2 = private_key_to_pkcs8(imported_key)
                .unwrap_or_else(|e| panic!("Failed to export private key for {curve:?} : {e:?}"));

            assert_eq!(exported_bytes_2, exported_bytes);
        });
    }

    #[test]
    fn public_key_can_be_imported_and_exported() {
        SUPPORTED_CURVES.iter().copied().for_each(|curve| {
            let key_bytes = get_test_public_keys().get_key_from_curve(curve);

            let imported_key = pub_key_from_uncompressed(key_bytes.clone(), curve)
                .unwrap_or_else(|e| panic!("Failed to import public key for {curve:?} : {e:?}"));

            let exported_bytes = pub_key_to_uncompressed(imported_key)
                .unwrap_or_else(|e| panic!("Failed to export public key for {curve:?} : {e:?}"));

            assert_eq!(exported_bytes, key_bytes);
        });
    }

    #[test]
    fn secret_to_public() {
        let test_public_keys = get_test_public_keys();
        let test_secret_keys = get_test_secret_keys();

        for curve in SUPPORTED_CURVES.iter().copied() {
            let secret_key = test_secret_keys.get_key_from_curve(curve);
            let public_key = private_key_bytes_to_public(secret_key, curve).unwrap();
            let expected_public_key = test_public_keys.get_key_from_curve(curve);
            assert_eq!(public_key, expected_public_key);
        }
    }

    // #[test]
    // fn mismatched_curve_import() {
    //     for curve in SUPPORTED_CURVES.iter().copied() {
    //         for other_curve in SUPPORTED_CURVES
    //             .iter()
    //             .copied()
    //             .filter(|c| !byte_equal(*c, curve))
    //         {
    //             let public_key = get_test_public_keys().get_key_from_curve(curve);
    //             let res = pub_key_from_uncompressed(public_key, other_curve);

    //             assert!(res.is_err());
    //         }
    //     }
    // }

    // TODO: discuss if we need this test
    // TODO: if yes, we need to introduce secure cmp

    // #[test]
    // fn test_order_range_enforcement() {
    //     let p256_order =
    //         hex::decode("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
    //             .unwrap();

    //     // Keys must be <= to order
    //     let p256_res = private_key_from_bytes(p256_order, Curve::P256);
    //     assert_matches!(p256_res, Err(EcError::EcKeyInvalidKeyData));

    //     let nist_curves = [Curve::P256];

    //     // Keys must not be 0
    //     for curve in nist_curves {
    //         assert_matches!(
    //             private_key_from_bytes(vec![0u8; curve.secret_key_size()], curve),
    //             Err(EcError::EcKeyInvalidKeyData)
    //         );
    //     }
    // }

    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestCaseSignature {
        pub algorithm: String,
        #[serde(with = "hex::serde")]
        pub private_key: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub public_key: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub hash: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub signature: Vec<u8>,
    }

    fn test_sign_p256(private_key: Vec<u8>, public_key: Vec<u8>, data: Vec<u8>) {
        let curve = Curve::P256;
        let private_key = private_key_from_bytes(private_key, curve).unwrap();
        let public_key = pub_key_from_uncompressed(public_key, curve).unwrap();
        match private_key {
            super::EcPrivateKey::P256(private_key) => {
                let signature = sign_p256(private_key, &data).unwrap();
                match public_key {
                    EcPublicKey::P256(public_key) => {
                        let verify = verify_p256(public_key, &signature, &data).unwrap();
                        assert_eq!(verify, true)
                    }
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }
    }

    fn test_sign_ed25519(
        private_key: Vec<u8>,
        public_key: Vec<u8>,
        data: Vec<u8>,
        expected_signature: Vec<u8>,
    ) {
        let curve = Curve::Ed25519;
        let private_key = private_key_from_bytes(private_key, curve).unwrap();
        let public_key = pub_key_from_uncompressed(public_key, curve).unwrap();
        match private_key {
            super::EcPrivateKey::Ed25519(private_key) => {
                let signature = sign_ed25519(private_key, &data).unwrap();
                assert_eq!(signature, expected_signature);
                match public_key {
                    EcPublicKey::Ed25519(public_key) => {
                        let verify = verify_ed25519(public_key, &signature, &data).unwrap();
                        assert_eq!(verify, true)
                    }
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn test_ecdsa_eddsa() {
        let test_case_file = include_str!("../test_data/test_ecdsa_eddsa.json");
        let test_cases: Vec<TestCaseSignature> = serde_json::from_str(test_case_file).unwrap();

        for case in test_cases {
            match case.algorithm.as_str() {
                "ECDSA" => test_sign_p256(case.private_key, case.public_key, case.hash),
                "EDDSA" => {
                    test_sign_ed25519(case.private_key, case.public_key, case.hash, case.signature)
                }
                _ => assert!(false),
            }
        }
    }
}
