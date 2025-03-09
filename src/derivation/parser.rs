use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::str::FromStr;

use crate::builtins::hash::Algorithm;

use super::hash::Hash;
use super::{ContentAddressMethod, Derivation, DerivationOutput};

#[derive(Debug)]
pub enum DerivationParseError {
    /// There should not be info about the SyntaxError
    /// because it means that something is corrupted
    /// or modified by an external.
    SyntaxError,

    CANoPath,
    Expected(&'static str),
    ImpureNoPath,
    InvalidBase16Hash(String),
    InvalidBase32Hash(String),
    InvalidBase64Hash(String),
    InvalidHashLength(String, Algorithm),
    InvalidPath(PathBuf),
    UnknownHashAlgorithm(String),
    UnterminatedString,
}

type Input<'a, 'b> = &'a mut &'b str;

impl FromStr for Derivation {
    type Err = DerivationParseError;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        let haystack = &mut s;

        expect(haystack, "Derive(")?;

        let outputs = parse_outputs(haystack)?;
        expect(haystack, ",")?;
        let input_derivations = parse_input_derivations(haystack)?;
        expect(haystack, ",")?;
        let input_sources = parse_list_of_strings(haystack)?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        expect(haystack, ",")?;
        let platform = parse_string(haystack)?;
        expect(haystack, ",")?;
        let builder = parse_string(haystack)?.into();
        expect(haystack, ",")?;
        let args = parse_list_of_strings(haystack)?;
        expect(haystack, ",")?;
        let env = parse_env(haystack)?;
        expect(haystack, ")")?;

        // FIXME: This should be given from `.drv` path
        let name = env
            .iter()
            .find_map(|(key, value)| (key == "name").then_some(value))
            .expect("Derivation should have `name` variable")
            .clone();

        Ok(Derivation {
            outputs,
            input_derivations,
            input_sources,
            platform,
            builder,
            args,
            env,
            name,
        })
    }
}

impl DerivationOutput {
    pub fn parse(
        path: String,
        algorithm: String,
        hash: String,
    ) -> Result<Self, DerivationParseError> {
        if !algorithm.is_empty() {
            let algorithm = &mut algorithm.as_str();
            let method = ContentAddressMethod::parse(algorithm);
            let algorithm = Algorithm::parse(algorithm)?;

            if hash == "impure" {
                if !path.is_empty() {
                    return Err(DerivationParseError::ImpureNoPath);
                }
                Ok(DerivationOutput::Impure { method, algorithm })
            } else if !hash.is_empty() {
                let path = PathBuf::from(path);
                if path.is_relative() {
                    return Err(DerivationParseError::InvalidPath(path));
                }
                let hash = Hash::new(hash, algorithm, false)?;
                Ok(DerivationOutput::CAFixed { method, hash })
            } else {
                if !path.is_empty() {
                    return Err(DerivationParseError::CANoPath);
                }
                Ok(DerivationOutput::CAFloating { method, algorithm })
            }
        } else {
            if path.is_empty() {
                return Ok(Self::Deferred);
            }
            let path = PathBuf::from(path);
            if path.is_relative() {
                return Err(DerivationParseError::InvalidPath(path));
            }
            Ok(Self::InputAddressed(
                path.file_stem()
                    .expect("Empty path is deferred")
                    .to_string_lossy()
                    .into_owned(),
            ))
        }
    }
}

impl ContentAddressMethod {
    pub fn parse(haystack: Input) -> Self {
        if skip_peek(haystack, "r:") {
            Self::NixArchive
        } else if skip_peek(haystack, "git:") {
            Self::Git
        } else if skip_peek(haystack, "text:") {
            Self::Text
        } else {
            Self::Flat
        }
    }
}

impl Algorithm {
    pub fn parse(haystack: Input) -> Result<Self, DerivationParseError> {
        match *haystack {
            "md5" => Ok(Self::MD5),
            "sha1" => Ok(Self::SHA1),
            "sha256" => Ok(Self::SHA256),
            "sha512" => Ok(Self::SHA512),
            _ => Err(DerivationParseError::UnknownHashAlgorithm(
                haystack.to_owned(),
            )),
        }
    }
}

fn parse_outputs(
    haystack: Input,
) -> Result<BTreeMap<String, DerivationOutput>, DerivationParseError> {
    parse_list_fold(haystack, |haystack| {
        let (id, path, algorithm, hash) = parse_tuple_4(haystack)?;
        let output = DerivationOutput::parse(path, algorithm, hash)?;

        Ok((id, output))
    })
}

fn parse_input_derivations(
    haystack: Input,
) -> Result<BTreeMap<String, Vec<String>>, DerivationParseError> {
    parse_list_fold(haystack, |haystack| {
        expect(haystack, "(")?;
        let id = parse_string(haystack)?;
        expect(haystack, ",")?;
        let outputs = parse_list_of_strings(haystack)?;
        expect(haystack, ")")?;

        Ok((id, outputs))
    })
}

fn parse_env(haystack: Input) -> Result<Vec<(String, String)>, DerivationParseError> {
    parse_list_fold(haystack, |haystack| parse_tuple_2(haystack))
}

// ===== Parsing Utils ===== //

fn parse_list_of_strings(haystack: Input) -> Result<Vec<String>, DerivationParseError> {
    parse_list_fold(haystack, |haystack| parse_string(haystack))
}

fn parse_list_fold<R, B: Default + Extend<R>>(
    haystack: Input,
    mut item: impl FnMut(&mut &str) -> Result<R, DerivationParseError>,
) -> Result<B, DerivationParseError> {
    expect(haystack, "[")?;

    let mut accum = B::default();

    while !end_of_list(haystack) {
        match item(haystack) {
            Ok(item) => accum.extend(Some(item)),
            Err(e) => return Err(e),
        }
    }

    Ok(accum)
}

fn parse_list(
    haystack: Input,
    mut item: impl FnMut(&mut &str) -> Result<(), DerivationParseError>,
) -> Result<(), DerivationParseError> {
    expect(haystack, "[")?;

    while !end_of_list(haystack) {
        item(haystack)?;
    }

    Ok(())
}

fn parse_tuple_2(haystack: Input) -> Result<(String, String), DerivationParseError> {
    expect(haystack, "(")?;
    let a = parse_string(haystack)?;
    expect(haystack, ",")?;
    let b = parse_string(haystack)?;
    expect(haystack, ")")?;

    Ok((a, b))
}

fn parse_tuple_4(
    haystack: Input,
) -> Result<(String, String, String, String), DerivationParseError> {
    expect(haystack, "(")?;
    let a = parse_string(haystack)?;
    expect(haystack, ",")?;
    let b = parse_string(haystack)?;
    expect(haystack, ",")?;
    let c = parse_string(haystack)?;
    expect(haystack, ",")?;
    let d = parse_string(haystack)?;
    expect(haystack, ")")?;

    Ok((a, b, c, d))
}

fn parse_string(haystack: Input) -> Result<String, DerivationParseError> {
    expect(haystack, "\"")?;

    let ControlFlow::Break(content_len) =
        haystack
            .chars()
            .try_fold((0, false), |(count, escaped), c| match (c, escaped) {
                // Toggle escaped
                ('\\', false) => ControlFlow::Continue((count, true)),

                // End of string
                ('"', false) => ControlFlow::Break(count),

                // Ignore other characters
                _ => ControlFlow::Continue((count + 1, false)),
            })
    else {
        return Err(DerivationParseError::UnterminatedString);
    };

    let mut output = String::with_capacity(content_len);
    let mut escaped = false;

    let mut haystack_chars = haystack.chars();

    while let Some(c) = haystack_chars.next() {
        match (c, escaped) {
            ('\\', false) => {
                escaped = true;
                skip_unchecked(haystack, 1);
            }
            ('"', false) => break,

            _ => {
                skip_unchecked(haystack, 1);

                match (c, escaped) {
                    ('n', true) => output.push('\n'),
                    ('r', true) => output.push('\r'),
                    ('t', true) => output.push('\t'),

                    _ => output.push(c),
                }

                escaped = false;
            }
        }
    }

    expect(haystack, "\"")?;

    Ok(output)
}

fn check_peek(haystack: Input, needle: &'static str) -> bool {
    haystack.len() >= needle.len() && &haystack[..needle.len()] == needle
}

#[must_use]
fn expect(haystack: Input, needle: &'static str) -> Result<(), DerivationParseError> {
    check_peek(haystack, needle)
        .then(|| skip_unchecked(haystack, needle.len()))
        .ok_or(DerivationParseError::Expected(needle))
}

fn skip_peek(haystack: Input, needle: &'static str) -> bool {
    check_peek(haystack, needle)
        .then(|| skip_unchecked(haystack, needle.len()))
        .is_some()
}

fn skip_unchecked(haystack: Input, amount: usize) {
    *haystack = &haystack[amount..];
}

fn end_of_list(haystack: Input) -> bool {
    if skip_peek(haystack, "]") {
        true
    } else {
        skip_peek(haystack, ",");
        false
    }
}

// ========= TESTS ========= //

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::derivation::Derivation;

    #[test]
    fn real_derivation() {
        const EXPECTED: &str = r#"{
  "args": [
    "-e",
    "/nix/store/vj1c3wf9c11a0qs6p3ymfvrnsdgsdcbq-source-stdenv.sh",
    "/nix/store/mkmplw6zphafsxz2hpsvjv3fyd5qn0ad-builder.sh"
  ],
  "builder": "/nix/store/57yw47n69c0x4c3vnyfb4ilqbidx77jp-bash-5.2p37/bin/bash",
  "env": {
    "SSL_CERT_FILE": "/no-cert-file.crt",
    "__structuredAttrs": "",
    "buildInputs": "",
    "builder": "/nix/store/57yw47n69c0x4c3vnyfb4ilqbidx77jp-bash-5.2p37/bin/bash",
    "cmakeFlags": "",
    "configureFlags": "",
    "curlOpts": "",
    "curlOptsList": "",
    "depsBuildBuild": "",
    "depsBuildBuildPropagated": "",
    "depsBuildTarget": "",
    "depsBuildTargetPropagated": "",
    "depsHostHost": "",
    "depsHostHostPropagated": "",
    "depsTargetTarget": "",
    "depsTargetTargetPropagated": "",
    "doCheck": "",
    "doInstallCheck": "",
    "downloadToTemp": "1",
    "executable": "",
    "impureEnvVars": "http_proxy https_proxy ftp_proxy all_proxy no_proxy HTTP_PROXY HTTPS_PROXY FTP_PROXY ALL_PROXY NO_PROXY NIX_SSL_CERT_FILE NIX_CURL_FLAGS NIX_HASHED_MIRRORS NIX_CONNECT_TIMEOUT NIX_MIRRORS_alsa NIX_MIRRORS_apache NIX_MIRRORS_bioc NIX_MIRRORS_bitlbee NIX_MIRRORS_centos NIX_MIRRORS_cpan NIX_MIRRORS_cran NIX_MIRRORS_debian NIX_MIRRORS_dub NIX_MIRRORS_fedora NIX_MIRRORS_gcc NIX_MIRRORS_gentoo NIX_MIRRORS_gnome NIX_MIRRORS_gnu NIX_MIRRORS_gnupg NIX_MIRRORS_hackage NIX_MIRRORS_hashedMirrors NIX_MIRRORS_ibiblioPubLinux NIX_MIRRORS_imagemagick NIX_MIRRORS_kde NIX_MIRRORS_kernel NIX_MIRRORS_luarocks NIX_MIRRORS_maven NIX_MIRRORS_mozilla NIX_MIRRORS_mysql NIX_MIRRORS_openbsd NIX_MIRRORS_opensuse NIX_MIRRORS_osdn NIX_MIRRORS_postgresql NIX_MIRRORS_pypi NIX_MIRRORS_qt NIX_MIRRORS_sageupstream NIX_MIRRORS_samba NIX_MIRRORS_savannah NIX_MIRRORS_sourceforge NIX_MIRRORS_steamrt NIX_MIRRORS_tcsh NIX_MIRRORS_testpypi NIX_MIRRORS_ubuntu NIX_MIRRORS_xfce NIX_MIRRORS_xorg",
    "mesonFlags": "",
    "mirrorsFile": "/nix/store/g3w9rz290a64j5f84g6khwwm6qsqs6x9-mirrors-list",
    "name": "source",
    "nativeBuildInputs": "/nix/store/jkfpzd5dsfk8cc5ndmk1wq2m7w18702m-curl-8.11.1-dev",
    "nixpkgsVersion": "25.05",
    "out": "/nix/store/nyrnk08phhlwsps94irya05y6hz8r3jh-source",
    "outputHash": "sha256-ir4hG2VIPv3se7JfWqCM/siLqFEFkmhMW/IGCocy6Pc=",
    "outputHashMode": "recursive",
    "outputs": "out",
    "patches": "",
    "postFetch": "unpackDir=\"$TMPDIR/unpack\"\nmkdir \"$unpackDir\"\ncd \"$unpackDir\"\n\nrenamed=\"$TMPDIR/download.tar.gz\"\nmv \"$downloadedFile\" \"$renamed\"\nunpackFile \"$renamed\"\nchmod -R +w \"$unpackDir\"\nif [ $(ls -A \"$unpackDir\" | wc -l) != 1 ]; then\n  echo \"error: zip file must contain a single file or directory.\"\n  echo \"hint: Pass stripRoot=false; to fetchzip to assume flat list of files.\"\n  exit 1\nfi\nfn=$(cd \"$unpackDir\" && ls -A)\nif [ -f \"$unpackDir/$fn\" ]; then\n  mkdir $out\nfi\nmv \"$unpackDir/$fn\" \"$out\"\n\n\nchmod 755 \"$out\"\n",
    "preferHashedMirrors": "1",
    "preferLocalBuild": "1",
    "propagatedBuildInputs": "",
    "propagatedNativeBuildInputs": "",
    "showURLs": "",
    "stdenv": "/nix/store/sqlqg4xpjx6vwp035arafzcb2xgy5d08-stdenv-linux",
    "strictDeps": "",
    "system": "i686-linux",
    "urls": "https://github.com/abseil/abseil-cpp/archive/refs/tags/20240722.1.tar.gz"
  },
  "inputDrvs": {
    "/nix/store/34m64sj9widbc8xplj7ksis9lqwxxlnr-stdenv-linux.drv": {
      "dynamicOutputs": {},
      "outputs": [
        "out"
      ]
    },
    "/nix/store/br4zn4cxb7qxalgca56h27p2f7vz0xjq-curl-8.11.1.drv": {
      "dynamicOutputs": {},
      "outputs": [
        "dev"
      ]
    },
    "/nix/store/jywsfahl2vflkk0vf6b2jf1awix2lq1d-bash-5.2p37.drv": {
      "dynamicOutputs": {},
      "outputs": [
        "out"
      ]
    },
    "/nix/store/m5zqmrbywb9fy3m4csngy48d52iyhsj8-mirrors-list.drv": {
      "dynamicOutputs": {},
      "outputs": [
        "out"
      ]
    }
  },
  "inputSrcs": [
    "/nix/store/mkmplw6zphafsxz2hpsvjv3fyd5qn0ad-builder.sh",
    "/nix/store/vj1c3wf9c11a0qs6p3ymfvrnsdgsdcbq-source-stdenv.sh"
  ],
  "name": "source",
  "outputs": {
    "out": {
      "hash": "8abe211b65483efdec7bb25f5aa08cfec88ba8510592684c5bf2060a8732e8f7",
      "hashAlgo": "sha256",
      "method": "nar",
      "path": "/nix/store/nyrnk08phhlwsps94irya05y6hz8r3jh-source"
    }
  },
  "system": "i686-linux"
}"#;

        let content = r#"Derive([("out","/nix/store/nyrnk08phhlwsps94irya05y6hz8r3jh-source","r:sha256","8abe211b65483efdec7bb25f5aa08cfec88ba8510592684c5bf2060a8732e8f7")],[("/nix/store/34m64sj9widbc8xplj7ksis9lqwxxlnr-stdenv-linux.drv",["out"]),("/nix/store/br4zn4cxb7qxalgca56h27p2f7vz0xjq-curl-8.11.1.drv",["dev"]),("/nix/store/jywsfahl2vflkk0vf6b2jf1awix2lq1d-bash-5.2p37.drv",["out"]),("/nix/store/m5zqmrbywb9fy3m4csngy48d52iyhsj8-mirrors-list.drv",["out"])],["/nix/store/mkmplw6zphafsxz2hpsvjv3fyd5qn0ad-builder.sh","/nix/store/vj1c3wf9c11a0qs6p3ymfvrnsdgsdcbq-source-stdenv.sh"],"i686-linux","/nix/store/57yw47n69c0x4c3vnyfb4ilqbidx77jp-bash-5.2p37/bin/bash",["-e","/nix/store/vj1c3wf9c11a0qs6p3ymfvrnsdgsdcbq-source-stdenv.sh","/nix/store/mkmplw6zphafsxz2hpsvjv3fyd5qn0ad-builder.sh"],[("SSL_CERT_FILE","/no-cert-file.crt"),("__structuredAttrs",""),("buildInputs",""),("builder","/nix/store/57yw47n69c0x4c3vnyfb4ilqbidx77jp-bash-5.2p37/bin/bash"),("cmakeFlags",""),("configureFlags",""),("curlOpts",""),("curlOptsList",""),("depsBuildBuild",""),("depsBuildBuildPropagated",""),("depsBuildTarget",""),("depsBuildTargetPropagated",""),("depsHostHost",""),("depsHostHostPropagated",""),("depsTargetTarget",""),("depsTargetTargetPropagated",""),("doCheck",""),("doInstallCheck",""),("downloadToTemp","1"),("executable",""),("impureEnvVars","http_proxy https_proxy ftp_proxy all_proxy no_proxy HTTP_PROXY HTTPS_PROXY FTP_PROXY ALL_PROXY NO_PROXY NIX_SSL_CERT_FILE NIX_CURL_FLAGS NIX_HASHED_MIRRORS NIX_CONNECT_TIMEOUT NIX_MIRRORS_alsa NIX_MIRRORS_apache NIX_MIRRORS_bioc NIX_MIRRORS_bitlbee NIX_MIRRORS_centos NIX_MIRRORS_cpan NIX_MIRRORS_cran NIX_MIRRORS_debian NIX_MIRRORS_dub NIX_MIRRORS_fedora NIX_MIRRORS_gcc NIX_MIRRORS_gentoo NIX_MIRRORS_gnome NIX_MIRRORS_gnu NIX_MIRRORS_gnupg NIX_MIRRORS_hackage NIX_MIRRORS_hashedMirrors NIX_MIRRORS_ibiblioPubLinux NIX_MIRRORS_imagemagick NIX_MIRRORS_kde NIX_MIRRORS_kernel NIX_MIRRORS_luarocks NIX_MIRRORS_maven NIX_MIRRORS_mozilla NIX_MIRRORS_mysql NIX_MIRRORS_openbsd NIX_MIRRORS_opensuse NIX_MIRRORS_osdn NIX_MIRRORS_postgresql NIX_MIRRORS_pypi NIX_MIRRORS_qt NIX_MIRRORS_sageupstream NIX_MIRRORS_samba NIX_MIRRORS_savannah NIX_MIRRORS_sourceforge NIX_MIRRORS_steamrt NIX_MIRRORS_tcsh NIX_MIRRORS_testpypi NIX_MIRRORS_ubuntu NIX_MIRRORS_xfce NIX_MIRRORS_xorg"),("mesonFlags",""),("mirrorsFile","/nix/store/g3w9rz290a64j5f84g6khwwm6qsqs6x9-mirrors-list"),("name","source"),("nativeBuildInputs","/nix/store/jkfpzd5dsfk8cc5ndmk1wq2m7w18702m-curl-8.11.1-dev"),("nixpkgsVersion","25.05"),("out","/nix/store/nyrnk08phhlwsps94irya05y6hz8r3jh-source"),("outputHash","sha256-ir4hG2VIPv3se7JfWqCM/siLqFEFkmhMW/IGCocy6Pc="),("outputHashMode","recursive"),("outputs","out"),("patches",""),("postFetch","unpackDir=\"$TMPDIR/unpack\"\nmkdir \"$unpackDir\"\ncd \"$unpackDir\"\n\nrenamed=\"$TMPDIR/download.tar.gz\"\nmv \"$downloadedFile\" \"$renamed\"\nunpackFile \"$renamed\"\nchmod -R +w \"$unpackDir\"\nif [ $(ls -A \"$unpackDir\" | wc -l) != 1 ]; then\n  echo \"error: zip file must contain a single file or directory.\"\n  echo \"hint: Pass stripRoot=false; to fetchzip to assume flat list of files.\"\n  exit 1\nfi\nfn=$(cd \"$unpackDir\" && ls -A)\nif [ -f \"$unpackDir/$fn\" ]; then\n  mkdir $out\nfi\nmv \"$unpackDir/$fn\" \"$out\"\n\n\nchmod 755 \"$out\"\n"),("preferHashedMirrors","1"),("preferLocalBuild","1"),("propagatedBuildInputs",""),("propagatedNativeBuildInputs",""),("showURLs",""),("stdenv","/nix/store/sqlqg4xpjx6vwp035arafzcb2xgy5d08-stdenv-linux"),("strictDeps",""),("system","i686-linux"),("urls","https://github.com/abseil/abseil-cpp/archive/refs/tags/20240722.1.tar.gz")])"#;

        let parsed = Derivation::from_str(content).unwrap();

        assert_eq!(format!("{parsed}"), EXPECTED);
    }
}
