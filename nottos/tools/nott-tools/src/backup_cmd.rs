// SPDX-License-Identifier: Apache-2.0

//! `backup key-create`, `backup encrypt` and `backup decrypt`
//! (`docs/adr/0013-backup-format.md`).

use crate::args::Args;
use crate::util::read_text;
use crate::{CommandResult, Outcome};
use nott_backup::{BackupError, BackupKey, Decryptor, DEFAULT_PARAMS, MAX_SECRET_LEN};
use nott_crypto::password::Argon2idParams;
use nott_format::text::parse_decimal;
use nott_format::time::Timestamp;
use std::fs::{self, File};
use std::io::{BufWriter, Read as _, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

const MAX_KEY_FILE_BYTES: u64 = 8 * 1024;

/// The recovery secret: the file's bytes, without one final line feed.
fn read_secret(path: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let describe = |error: std::io::Error| format!("{path}: {error}");
    let mut secret = Zeroizing::new(Vec::new());
    File::open(path)
        .map_err(describe)?
        .take(MAX_SECRET_LEN as u64 + 2)
        .read_to_end(&mut secret)
        .map_err(describe)?;
    if secret.last() == Some(&b'\n') {
        secret.pop();
    }
    Ok(secret)
}

/// Creates a file that must not exist yet, readable by its owner only.
fn create_private(path: &Path) -> Result<File, String> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn cost(args: &Args<'_>, name: &str, default: u32) -> Result<u32, String> {
    match args.optional(name)? {
        None => Ok(default),
        Some(text) => parse_decimal(text)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("--{name} must be a whole number without leading zeros")),
    }
}

/// A refusal to make or read a backup is a failed check; anything else
/// means the check could not run.
fn outcome(error: BackupError) -> CommandResult {
    match error {
        BackupError::Io(error) => Err(error.to_string()),
        BackupError::OutOfMemory | BackupError::Randomness => Err(error.to_string()),
        refusal => Ok(Outcome::Fail(format!("refused: {refusal}\n"))),
    }
}

pub(crate) fn key_create(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &["secret-file", "memory-kib", "iterations", "lanes", "out"],
    )?;
    let [] = args.positionals()?;
    let secret = read_secret(args.required("secret-file")?)?;
    let params = Argon2idParams {
        memory_kib: cost(&args, "memory-kib", DEFAULT_PARAMS.memory_kib)?,
        iterations: cost(&args, "iterations", DEFAULT_PARAMS.iterations)?,
        lanes: cost(&args, "lanes", DEFAULT_PARAMS.lanes)?,
    };
    let out = args.required("out")?;
    let key = match BackupKey::create(&secret, params) {
        Ok(key) => key,
        Err(error) => return outcome(error),
    };
    create_private(Path::new(out))?
        .write_all(key.to_text().as_bytes())
        .map_err(|error| format!("{out}: {error}"))?;
    Ok(Outcome::Pass(format!(
        "wrote {out} (secret: it makes backups that the recovery secret's holder will accept)\n\
         recovery-key-id: {}\nkdf: {}\n",
        key.recovery_key_id(),
        key.kdf()
    )))
}

pub(crate) fn encrypt(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["key", "created", "out"])?;
    let [input] = args.positionals()?;
    let key_path = args.required("key")?;
    let key = match BackupKey::parse(&read_text(Path::new(key_path), MAX_KEY_FILE_BYTES)?) {
        Ok(key) => key,
        Err(error) => return Err(format!("{key_path}: {error}")),
    };
    let created = match args.optional("created")? {
        Some(text) => Timestamp::parse(text)
            .ok_or_else(|| "--created must look like 2026-11-14T00:00:00Z".to_owned())?,
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|now| Timestamp::from_unix(now.as_secs()))
            .ok_or_else(|| "the system clock is outside 1970 to 9999".to_owned())?,
    };
    let out = Path::new(args.required("out")?);
    let source = File::open(input).map_err(|error| format!("{input}: {error}"))?;
    let mut sink = BufWriter::new(create_private(out)?);
    let result = nott_backup::encrypt(&key, created, source, &mut sink);
    let result = result.and_then(|written| {
        sink.flush()?;
        Ok(written)
    });
    match result {
        Ok(written) => Ok(Outcome::Pass(format!(
            "wrote {}: {written} byte(s) of plaintext\nrecovery-key-id: {}\ncreated: {created}\n",
            out.display(),
            key.recovery_key_id()
        ))),
        Err(error) => {
            drop(sink);
            let _ = fs::remove_file(out);
            outcome(error)
        }
    }
}

pub(crate) fn decrypt(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["secret-file", "out"])?;
    let [input] = args.positionals()?;
    let secret = read_secret(args.required("secret-file")?)?;
    let out = Path::new(args.required("out")?);
    if out.exists() {
        return Err(format!("{}: already exists", out.display()));
    }
    let source = File::open(input).map_err(|error| format!("{input}: {error}"))?;
    let decryptor = match Decryptor::open(&secret, source) {
        Ok(decryptor) => decryptor,
        Err(error) => return outcome(error),
    };
    let created = decryptor.created();
    // The plaintext is written beside the destination and moved there only
    // once the whole backup has been authenticated.
    let mut partial = out.as_os_str().to_owned();
    partial.push(".partial");
    let partial = Path::new(&partial);
    let mut sink = BufWriter::new(create_private(partial)?);
    let result = decryptor.decrypt_to(&mut sink).and_then(|read| {
        sink.flush()?;
        Ok(read)
    });
    drop(sink);
    match result {
        Ok(read) => {
            fs::rename(partial, out).map_err(|error| format!("{}: {error}", out.display()))?;
            Ok(Outcome::Pass(format!(
                "wrote {}: {read} byte(s)\ncreated: {created}\n",
                out.display()
            )))
        }
        Err(error) => {
            let _ = fs::remove_file(partial);
            outcome(error)
        }
    }
}
