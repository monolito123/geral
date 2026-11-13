// SPDX-License-Identifier: Apache-2.0

//! `key generate`, `key id`, `sign` and `verify`: hybrid signatures over
//! files (`docs/adr/0008-signature-suite.md`).

use crate::args::Args;
use crate::util::{read_text, write_text};
use crate::{CommandResult, Outcome};
use nott_crypto::{PublicKey, SecretKey, Signature, Suite};
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;
/// Largest file `sign` and `verify` read into memory.
const MAX_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn load_public_key(path: &str) -> Result<PublicKey, String> {
    let text = read_text(Path::new(path), MAX_KEY_FILE_BYTES)?;
    PublicKey::parse(&text).map_err(|error| format!("{path}: {error}"))
}

pub(crate) fn load_signature(path: &str) -> Result<Signature, String> {
    let text = read_text(Path::new(path), MAX_KEY_FILE_BYTES)?;
    Signature::parse(&text).map_err(|error| format!("{path}: {error}"))
}

fn read_message(path: &str) -> Result<Vec<u8>, String> {
    let describe = |error: std::io::Error| format!("{path}: {error}");
    if fs::metadata(path).map_err(describe)?.len() > MAX_MESSAGE_BYTES {
        return Err(format!("{path}: larger than {MAX_MESSAGE_BYTES} bytes"));
    }
    fs::read(path).map_err(describe)
}

pub(crate) fn key_generate(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["suite", "out-secret", "out-public"])?;
    let [] = args.positionals()?;
    let secret_path = args.required("out-secret")?;
    let public_path = args.required("out-public")?;
    let suite = match args.optional("suite")? {
        None => Suite::Ed25519MlDsa87,
        Some(name) => Suite::from_name(name).ok_or_else(|| {
            let known: Vec<&str> = Suite::ALL.iter().map(|suite| suite.name()).collect();
            format!("unknown suite `{name}`; known: {}", known.join(", "))
        })?,
    };

    let key = SecretKey::generate(suite).map_err(|error| error.to_string())?;
    let public = key.public_key();
    // The secret key is readable by its owner only, and an existing file is
    // never overwritten.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(secret_path)
        .map_err(|error| format!("{secret_path}: {error}"))?;
    file.write_all(key.to_text().as_bytes())
        .map_err(|error| format!("{secret_path}: {error}"))?;
    write_text(Path::new(public_path), &public.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {secret_path} (secret) and {public_path}\nsuite: {}\nkey-id: {}\n",
        suite.name(),
        public.key_id()
    )))
}

pub(crate) fn key_id(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &[])?;
    let [path] = args.positionals()?;
    let public = load_public_key(path)?;
    Ok(Outcome::Pass(format!(
        "suite: {}\nkey-id: {}\n",
        public.suite().name(),
        public.key_id()
    )))
}

pub(crate) fn sign(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["key", "context", "out"])?;
    let [path] = args.positionals()?;
    let key_path = args.required("key")?;
    let key_text = zeroize::Zeroizing::new(read_text(Path::new(key_path), MAX_KEY_FILE_BYTES)?);
    let key = SecretKey::parse(&key_text).map_err(|error| format!("{key_path}: {error}"))?;
    let signature = key
        .sign(args.required("context")?, &read_message(path)?)
        .map_err(|error| error.to_string())?;
    let out = args.required("out")?;
    write_text(Path::new(out), &signature.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {out}\ncontext: {}\nkey-id: {}\n",
        signature.context(),
        signature.key_id()
    )))
}

pub(crate) fn verify(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["public-key", "signature", "context"])?;
    let [path] = args.positionals()?;
    let public = load_public_key(args.required("public-key")?)?;
    let signature = load_signature(args.required("signature")?)?;
    let context = args.required("context")?;
    match nott_crypto::verify(&public, context, &read_message(path)?, &signature) {
        Ok(()) => Ok(Outcome::Pass(format!(
            "PASS: valid {} signature for `{context}` by key {}\n",
            public.suite().name(),
            public.key_id()
        ))),
        Err(error) => Ok(Outcome::Fail(format!("FAIL: {error}\n"))),
    }
}
