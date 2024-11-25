//! Inspired by https://github.com/malept/crypto-hash/blob/master/src/imp/openssl.rs

use std::io::{self, Write};

use openssl::hash;

/// Available cryptographic hash functions.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Algorithm {
    /// Popular message digest algorithm, only available for backwards compatibility purposes.
    MD5,
    /// SHA-1 algorithm from NIST FIPS, only available for backwards compatibility purposes.
    SHA1,
    /// SHA-2 family algorithm (256 bits).
    SHA256,
    /// SHA-2 family algorithm (512 bits).
    SHA512,
}

/// Function for `Hasher` which generates a cryptographic digest serialized in
/// hexadecimal from the given data and algorithm.
pub fn hex_digest(algorithm: Algorithm, data: &[u8]) -> String {
    let mut hasher = Hasher::new(algorithm);
    hasher.write_all(data).expect("Could not write hash data");
    let hash = hasher.finish();
    hex::encode(hash)
}

/// Generator of digests using a cryptographic hash function.
pub struct Hasher(hash::Hasher);

impl Hasher {
    /// Create a new `Hasher` for the given `Algorithm`.
    pub fn new(algorithm: Algorithm) -> Hasher {
        let hash_type = match algorithm {
            Algorithm::MD5 => hash::MessageDigest::md5(),
            Algorithm::SHA1 => hash::MessageDigest::sha1(),
            Algorithm::SHA256 => hash::MessageDigest::sha256(),
            Algorithm::SHA512 => hash::MessageDigest::sha512(),
        };

        match hash::Hasher::new(hash_type) {
            Ok(hasher) => Hasher(hasher),
            Err(error_stack) => panic!("OpenSSL error(s): {}", error_stack),
        }
    }

    /// Generate a digest from the data written to the `Hasher`.
    pub fn finish(&mut self) -> Vec<u8> {
        let Hasher(ref mut hasher) = *self;
        match hasher.finish() {
            Ok(digest) => digest.to_vec(),
            Err(error_stack) => panic!("OpenSSL error(s): {}", error_stack),
        }
    }
}

impl io::Write for Hasher {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Hasher(ref mut hasher) = *self;
        hasher.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let Hasher(ref mut hasher) = *self;
        hasher.flush()
    }
}
