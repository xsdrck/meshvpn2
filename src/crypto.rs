//! Cryptographic primitives and key management
//!
//! This module provides all the cryptographic functionality needed for MeshVPN:
//! - X25519 key exchange (similar to WireGuard)
//! - ChaCha20Poly1305 AEAD encryption
//! - Key derivation and management
//!

use std::fmt;

use x25519_dalek::{PublicKey as DalekPublicKey, StaticSecret, SharedSecret};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use sha2::{Sha256, Digest};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};

/// A X25519 secret key
#[derive(Clone)]
pub struct SecretKey {
    inner: StaticSecret,
}

impl SecretKey {
    /// Generate a new random secret key
    pub fn random() -> Self {
        Self {
            inner: StaticSecret::random_from_rng(OsRng),
        }
    }

    /// Create a secret key from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: StaticSecret::from(bytes),
        }
    }

    /// Get the raw bytes of this secret key
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.inner.as_bytes()
    }

    /// Get the corresponding public key
    pub fn public_key(&self) -> DalekPublicKey {
        DalekPublicKey::from(&self.inner)
    }

    /// Derive a shared secret with a peer's public key
    pub fn derive_shared_secret(&self, peer_public: &PublicKey) -> SharedSecret {
        self.inner.diffie_hellman(&peer_public.inner)
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretKey")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

/// A X25519 public key
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey {
    inner: DalekPublicKey,
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        Self::from_hex(&hex_str)
            .map_err(|_| serde::de::Error::custom("Invalid hex string for PublicKey"))
    }
}

impl PublicKey {
    /// Create a public key from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: DalekPublicKey::from(bytes),
        }
    }

    /// Get the raw bytes of this public key
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.inner.as_bytes()
    }

    /// Get the hex representation of this public key
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse a hex string into a public key
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|_| Error::Crypto("Invalid hex string".into()))?;
        
        if bytes.len() != 32 {
            return Err(Error::Crypto("Public key must be 32 bytes".into()));
        }
        
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl From<&SecretKey> for PublicKey {
    fn from(sk: &SecretKey) -> Self {
        Self { inner: sk.public_key() }
    }
}

/// A key pair containing both secret and public key
#[derive(Clone)]
pub struct KeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let secret_key = SecretKey::random();
        let public_key = PublicKey::from(&secret_key);
        Self {
            secret_key,
            public_key,
        }
    }

    /// Create a key pair from a secret key
    pub fn from_secret_key(secret_key: SecretKey) -> Self {
        let public_key = PublicKey::from(&secret_key);
        Self {
            secret_key,
            public_key,
        }
    }

    /// Get the secret key
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    /// Get the public key
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Derive a shared secret with a peer's public key
    pub fn derive_shared_secret(&self, peer_public: &PublicKey) -> SharedSecret {
        self.secret_key.inner.diffie_hellman(&peer_public.inner)
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("public_key", &self.public_key)
            .field("secret_key", &"[REDACTED]")
            .finish()
    }
}

/// A derived key for encryption
pub struct DerivedKey {
    key: [u8; 32],
}

impl DerivedKey {
    /// Derive a key from a shared secret and salt
    pub fn from_shared_secret(shared_secret: &SharedSecret, salt: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(salt);
        let result = hasher.finalize();
        
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        Self { key }
    }

    /// Create a new encryption context from this key
    pub fn to_encryption_context(&self) -> EncryptionContext {
        EncryptionContext::new(self.key)
    }

    /// Get the raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

/// A context for encryption and decryption
#[derive(Clone)]
pub struct EncryptionContext {
    cipher: ChaCha20Poly1305,
}

impl EncryptionContext {
    /// Create a new encryption context
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(&key.into()),
        }
    }

    /// Create a new encryption context from a DerivedKey
    pub fn from_derived_key(key: &DerivedKey) -> Self {
        Self::new(*key.as_bytes())
    }

    /// Encrypt data with the given nonce
    pub fn encrypt(&self, nonce: [u8; 12], plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&nonce);
        self.cipher.encrypt(nonce, plaintext)
            .map_err(|_| Error::Crypto("Encryption failed".into()))
    }

    /// Decrypt data with the given nonce
    pub fn decrypt(&self, nonce: [u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&nonce);
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|_| Error::Crypto("Decryption failed".into()))
    }

    /// Generate a random nonce
    pub fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    /// Incremental nonce generation
    pub fn increment_nonce(nonce: &mut [u8; 12]) {
        for i in (0..12).rev() {
            if nonce[i] < 255 {
                nonce[i] += 1;
                break;
            } else {
                nonce[i] = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_pair_generation() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        
        assert_ne!(alice.public_key(), bob.public_key());
    }

    #[test]
    fn test_diffie_hellman() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        
        let alice_shared = alice.derive_shared_secret(bob.public_key());
        let bob_shared = bob.derive_shared_secret(alice.public_key());
        
        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
    }

    #[test]
    fn test_encryption_decryption() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        
        let alice_shared = alice.derive_shared_secret(bob.public_key());
        let alice_key = DerivedKey::from_shared_secret(&alice_shared, b"test");
        
        let bob_shared = bob.derive_shared_secret(alice.public_key());
        let bob_key = DerivedKey::from_shared_secret(&bob_shared, b"test");
        
        assert_eq!(alice_key.as_bytes(), bob_key.as_bytes());
        
        let alice_context = alice_key.to_encryption_context();
        let bob_context = bob_key.to_encryption_context();
        
        let nonce = EncryptionContext::generate_nonce();
        let plaintext = b"Hello, world!";
        
        let ciphertext = alice_context.encrypt(nonce, plaintext).unwrap();
        let decrypted = bob_context.decrypt(nonce, &ciphertext).unwrap();
        
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_public_key_hex() {
        let key_pair = KeyPair::generate();
        let hex = key_pair.public_key().to_hex();
        let decoded = PublicKey::from_hex(&hex).unwrap();
        assert_eq!(key_pair.public_key().as_bytes(), decoded.as_bytes());
    }
}
