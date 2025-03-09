//! This is completely inspired in the original implementation,
//! it can be found [here](https://github.com/NixOS/nix/blob/master/src/libstore/derivations.cc).
//! The parsing errors and derivation structures are modified
//! to match the actual code, but it's basically the same

pub mod hash;
pub mod parser;

use core::fmt;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use hash::Hash;

use crate::builtins::hash::{Algorithm, Hasher};

// NOTE: Keep this ordered in the `.drv` way, as there appears
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    outputs: BTreeMap<String, DerivationOutput>,
    // (input_name, outputs[])
    input_derivations: BTreeMap<String, Vec<String>>,
    input_sources: Vec<PathBuf>,
    platform: String,
    builder: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivationOutput {
    Deferred,

    /// Content Address Floating
    CAFloating {
        method: ContentAddressMethod,
        algorithm: Algorithm,
    },

    /// Content Address Fixed
    CAFixed {
        method: ContentAddressMethod,
        hash: Hash,
    },

    Impure {
        method: ContentAddressMethod,
        algorithm: Algorithm,
    },

    InputAddressed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentAddressMethod {
    Flat,
    Git,

    /// NAR
    NixArchive,

    Text,
}

/// json formatted derivation
impl fmt::Display for Derivation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('{')?;
        f.write_char('\n')?;

        f.write_str("  \"args\": [\n")?;
        self.args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                f.write_fmt(format_args!(
                    "    {arg:?}{}\n",
                    // no trailing comma
                    idx.ne(&(self.args.len() - 1))
                        .then_some(",")
                        .unwrap_or_default()
                ))
            })
            .collect::<fmt::Result>()?;
        f.write_str("  ],\n")?;

        f.write_fmt(format_args!(
            "  \"builder\": {:?},\n",
            self.builder.display()
        ))?;

        f.write_str("  \"env\": {\n")?;
        self.env
            .iter()
            .enumerate()
            .map(|(idx, (k, v))| {
                f.write_fmt(format_args!(
                    "    {k:?}: {v:?}{}\n",
                    // no trailing comma
                    idx.ne(&(self.env.len() - 1))
                        .then_some(",")
                        .unwrap_or_default()
                ))
            })
            .collect::<fmt::Result>()?;
        f.write_str("  },\n")?;

        f.write_str("  \"inputDrvs\": {\n")?;
        self.input_derivations
            .iter()
            .enumerate()
            .map(|(idx, (k, v))| {
                f.write_fmt(format_args!("    {k:?}: {{\n"))?;

                f.write_str("      \"dynamicOutputs\": {},\n")?;
                f.write_str("      \"outputs\": [\n")?;
                v.iter()
                    .enumerate()
                    .map(|(idx, arg)| {
                        f.write_fmt(format_args!(
                            "        {arg:?}{}\n",
                            // no trailing comma
                            idx.ne(&(v.len() - 1)).then_some(",").unwrap_or_default()
                        ))
                    })
                    .collect::<fmt::Result>()?;
                f.write_str("      ]\n")?;

                f.write_fmt(format_args!(
                    "    }}{}\n",
                    // no trailing comma
                    idx.ne(&(self.input_derivations.len() - 1))
                        .then_some(",")
                        .unwrap_or_default()
                ))
            })
            .collect::<fmt::Result>()?;
        f.write_str("  },\n")?;

        f.write_str("  \"inputSrcs\": [\n")?;
        self.input_sources
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                f.write_fmt(format_args!(
                    "    {arg:?}{}\n",
                    // no trailing comma
                    idx.ne(&(self.input_sources.len() - 1))
                        .then_some(",")
                        .unwrap_or_default()
                ))
            })
            .collect::<fmt::Result>()?;
        f.write_str("  ],\n")?;

        f.write_fmt(format_args!("  \"name\": {:?},\n", self.name))?;

        f.write_str("  \"outputs\": {\n")?;
        self.outputs
            .iter()
            .enumerate()
            .map(|(idx, (k, v))| {
                f.write_fmt(format_args!("    {k:?}: {{\n"))?;

                match v {
                    DerivationOutput::Deferred => todo!(),
                    DerivationOutput::CAFloating { .. } => todo!(),
                    DerivationOutput::CAFixed { method, hash } => {
                        f.write_fmt(format_args!("      \"hash\": {:?},\n", hash.print_base16()))?;
                        f.write_fmt(format_args!(
                            "      \"hashAlgo\": {:?},\n",
                            match hash.algorithm {
                                Algorithm::MD5 => "md5",
                                Algorithm::SHA1 => "sha1",
                                Algorithm::SHA256 => "sha256",
                                Algorithm::SHA512 => "sha512",
                            }
                        ))?;
                        f.write_fmt(format_args!("      \"method\": \"{}\",\n", method))?;

                        let path_name = if k == "out" {
                            self.name.clone()
                        } else {
                            format!("{}-{k}", self.name)
                        };

                        if *method == ContentAddressMethod::Git && hash.algorithm != Algorithm::SHA1
                        {
                            // Git file ingestion must use SHA-1 hash
                            // https://github.com/NixOS/nix/blob/master/src/libstore/store-api.cc#L125
                            return Err(fmt::Error);
                        }

                        let path = if *method == ContentAddressMethod::NixArchive
                            && hash.algorithm == Algorithm::SHA256
                        {
                            let hash_part = {
                                let hashed = Hasher::new(Algorithm::SHA256)
                                    .finish_with(format!("source:{}:{}:/nix/store:{path_name}", hash.algorithm, hash.print_base16()).as_str().as_bytes());

                                let mut hash_part = Hash::new_empty(hash.algorithm.clone());
                                hash_part.hash_size = 20;

                                for i in 0..hash.hash_size {
                                    hash_part.hash[i % 20] ^= hashed[i];
                                }

                                hash_part.print_base32()
                            };

                            // FIXME: This should be able to change the nix store folder
                            format!("/nix/store/{hash_part}-{path_name}")
                        } else {
                            todo!()
                        };

                        f.write_fmt(format_args!("      \"path\": {:?}\n", path))?;
                    }
                    DerivationOutput::Impure { .. } => todo!(),
                    DerivationOutput::InputAddressed(_) => todo!(),
                }

                f.write_fmt(format_args!(
                    "    }}{}\n",
                    // no trailing comma
                    idx.ne(&(self.outputs.len() - 1))
                        .then_some(",")
                        .unwrap_or_default()
                ))
            })
            .collect::<fmt::Result>()?;
        f.write_str("  },\n")?;

        f.write_fmt(format_args!("  \"system\": {:?}\n", self.platform))?;

        f.write_char('}')
    }
}

impl fmt::Display for ContentAddressMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentAddressMethod::Flat => f.write_str("flat"),
            ContentAddressMethod::Git => f.write_str("git"),
            ContentAddressMethod::NixArchive => f.write_str("nar"),
            ContentAddressMethod::Text => f.write_str("text"),
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::MD5 => f.write_str("md5"),
            Algorithm::SHA1 => f.write_str("sha1"),
            Algorithm::SHA256 => f.write_str("sha256"),
            Algorithm::SHA512 => f.write_str("sha512"),
        }
    }
}
