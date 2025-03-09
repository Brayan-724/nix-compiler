//! Inspired by https://github.com/NixOS/nix/blob/master/src/libutil/hash.cc

use core::fmt;

use crate::builtins::hash::Algorithm;

use super::parser::DerivationParseError;

#[derive(Clone, PartialEq, Eq)]
pub struct Hash {
    pub algorithm: Algorithm,
    pub hash_size: usize,
    pub hash: [u8; Hash::MAX_HASH_SIZE],
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hash")
            .field("algorithm", &self.algorithm)
            .field("hash_size", &self.hash_size)
            .field("hash", &self.print_base16())
            .finish()
    }
}

impl Hash {
    pub const MAX_HASH_SIZE: usize = 64;

    // omitted: E O U T
    pub const NIX32_CHARS: &str = "0123456789abcdfghijklmnpqrsvwxyz";

    pub fn new(
        rest: String,
        algorithm: Algorithm,
        is_sri: bool,
    ) -> Result<Self, DerivationParseError> {
        let hash_size: usize = match algorithm {
            Algorithm::MD5 => 16,
            Algorithm::SHA1 => 20,
            Algorithm::SHA256 => 32,
            Algorithm::SHA512 => 64,
        };
        let mut hash_out = [0u8; Hash::MAX_HASH_SIZE];

        if !is_sri && rest.len() == Self::base16(hash_size) {
            let parse_hex_digit = |c: char| match c {
                '0'..='9' => Ok(c as u8 - '0' as u8),
                'A'..='F' => Ok(c as u8 - 'A' as u8 + 10),
                'a'..='f' => Ok(c as u8 - 'a' as u8 + 10),
                _ => Err(DerivationParseError::InvalidBase16Hash(rest.clone())),
            };

            for i in 0..hash_size {
                let mut str = rest[i * 2..].chars();
                hash_out[i] = parse_hex_digit(str.next().unwrap())? << 4
                    | parse_hex_digit(str.next().unwrap())?;
            }
        } else if !is_sri && rest.len() == Self::base32(hash_size) {
            // chars reversed but enumerated in acendant order
            for (n, c) in rest.chars().rev().enumerate() {
                let Some(digit @ ..32) =
                    Hash::NIX32_CHARS.chars().position(|nix_char| nix_char == c)
                else {
                    return Err(DerivationParseError::InvalidBase32Hash(rest));
                };

                let b = n * 5;
                let i = b / 8;
                let j = b % 8;
                hash_out[i] |= (digit as u8) << j;

                let the_magik_thingy_liringy = (digit as u8) >> (8 - j);
                if i < hash_size - 1 {
                    hash_out[i + 1] |= the_magik_thingy_liringy;
                } else if the_magik_thingy_liringy != 0 {
                    return Err(DerivationParseError::InvalidBase32Hash(rest));
                }
            }
        } else if is_sri || rest.len() == Self::base64(hash_size) {
            let Ok(d) = openssl::base64::decode_block(&rest) else {
                return Err(DerivationParseError::InvalidBase32Hash(rest));
            };

            let d = unsafe { String::from_utf8_unchecked(d) };

            if d.len() != hash_size {
                return Err(DerivationParseError::InvalidBase32Hash(rest));
            }

            let d = d[..hash_size].as_bytes();

            for idx in 0..Hash::MAX_HASH_SIZE {
                hash_out[idx] = d[idx];
            }
        } else {
            return Err(DerivationParseError::InvalidHashLength(rest, algorithm));
        }

        Ok(Self {
            algorithm,
            hash_size,
            hash: hash_out,
        })
    }

    pub fn new_empty(algorithm: Algorithm) -> Self {
        let hash_size: usize = match algorithm {
            Algorithm::MD5 => 16,
            Algorithm::SHA1 => 20,
            Algorithm::SHA256 => 32,
            Algorithm::SHA512 => 64,
        };

        Self {
            algorithm,
            hash_size,
            hash: [0u8; Hash::MAX_HASH_SIZE],
        }
    }

    pub const BASE16_CHARS: &str = "0123456789abcdef";

    pub fn print_base16(&self) -> String {
        let mut buf = String::with_capacity(self.len_base16());
        for hash_c in &self.hash[..self.hash_size] {
            buf.push(Self::BASE16_CHARS.as_bytes()[(hash_c >> 4) as usize] as char);
            buf.push(Self::BASE16_CHARS.as_bytes()[(hash_c & 0x0f) as usize] as char);
        }
        buf
    }

    // omitted: E O U T
    pub const BASE32_CHARS: &str = "0123456789abcdfghijklmnpqrsvwxyz";

    pub fn print_base32(&self) -> String {
        let len = self.len_base32();

        let mut s = String::with_capacity(len);

        for n in (0..len).rev() {
            let b = n * 5;
            let i = b / 8;
            let j = b % 8;
            let c = (self.hash[i] >> j) as usize
                | (if i >= self.hash_size - 1 {
                    0
                } else {
                    (self.hash[i + 1] as usize) << (8 - j)
                });
            s.push(Self::BASE32_CHARS.as_bytes()[(c & 0x1f) as usize] as char);
        }

        s
    }

    /// Returns the length of a base-16 representation of this hash.
    pub fn len_base16(&self) -> usize {
        Self::base16(self.hash_size)
    }

    /// Returns the length of a base-32 representation of this hash.
    pub fn len_base32(&self) -> usize {
        Self::base32(self.hash_size)
    }

    /// Returns the length of a base-64 representation of this hash.
    pub fn len_base64(&self) -> usize {
        Self::base64(self.hash_size)
    }

    /// Returns the length of a base-16 representation of this hash.
    pub fn base16(hash_size: usize) -> usize {
        hash_size * 2
    }

    /// Returns the length of a base-32 representation of this hash.
    pub fn base32(hash_size: usize) -> usize {
        (hash_size * 8 - 1) / 5 + 1
    }

    /// Returns the length of a base-64 representation of this hash.
    pub fn base64(hash_size: usize) -> usize {
        ((4 * hash_size / 3) + 3) & !3
    }
}

#[cfg(test)]
mod test {
    use crate::builtins::hash::{Algorithm, Hasher};

    use super::Hash;

    #[test]
    fn decode_store_hash() {
        const EXPECTED: &str = "nyrnk08phhlwsps94irya05y6hz8r3jh";

        let hashed = Hasher::new(Algorithm::SHA256)
            .finish_with("source:sha256:8abe211b65483efdec7bb25f5aa08cfec88ba8510592684c5bf2060a8732e8f7:/nix/store:source".as_bytes());

        let mut hash_part = Hash::new_empty(Algorithm::SHA256);
        hash_part.hash_size = 20;

        for i in 0..32 {
            hash_part.hash[i % 20] ^= hashed[i];
        }

        let hash_part = hash_part.print_base32();

        assert_eq!(hash_part, EXPECTED);
    }
}
