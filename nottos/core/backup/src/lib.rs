// SPDX-License-Identifier: Apache-2.0

//! NottOS backup encryption (`docs/adr/0013-backup-format.md`).
//!
//! A backup is a byte stream encrypted so that only the holder of the
//! **recovery secret** can read it, and authenticated so that nobody
//! without the device's **backup key** or the recovery secret can make one
//! that is accepted. The device can make backups without being able to
//! read them.
//!
//! # Construction
//!
//! ```text
//! recovery secret --Argon2id(salt, m, t, p)--> seed (32 bytes)
//! SHAKE256("NottOS-backup-recovery-v1" || seed) -> 128 bytes:
//!     x25519 secret (32) || ML-KEM-1024 seed d || z (64) || auth key (32)
//! recovery public key = public half of the hybrid KEM key (ADR-0012)
//! recovery key id     = SHA-256(recovery public key)
//!
//! each backup:
//!   data key K           = 32 random bytes
//!   (ct, ss)             = hybrid KEM encapsulation to the recovery public key
//!   wrap key             = SHA3-256("NottOS-backup-wrap-v1" || ss || auth key)
//!   wrapped key          = ChaCha20-Poly1305(wrap key, nonce 0^12,
//!                            aad = header up to and including the
//!                            kem-ciphertext line, plaintext K)
//!   payload key          = SHA3-256("NottOS-backup-payload-v1" || K)
//!   chunk i              = ChaCha20-Poly1305(payload key,
//!                            nonce = i as 11 bytes big endian || last,
//!                            aad = "", up to 64 KiB of plaintext)
//! ```
//!
//! `last` is 1 for the final chunk and 0 otherwise. Every chunk but the
//! final one holds exactly 64 KiB; the final one may be shorter, and is
//! empty only when the whole payload is. This is the payload layout of the
//! age format (C2SP `age.md`), with NottOS's own key schedule.
//!
//! # Reading
//!
//! [`Decryptor::open`] authenticates the whole header before anything in
//! it is used, apart from the cost parameters needed to derive the keys,
//! which are bounded first. [`Decryptor::decrypt_to`] releases each chunk
//! only after its tag verifies, but a stream cut short is detected only at
//! its end: whoever consumes the output must discard it unless
//! `decrypt_to` returns `Ok`.

use nott_crypto::aead::{self, AeadError, NONCE_LEN, TAG_LEN};
use nott_crypto::hash::{sha3_256, shake256};
use nott_crypto::kem::{self, KemCiphertext, KemError, KemPublicKey, KemSecretKey};
use nott_crypto::password::{self, Argon2idParams, KdfError};
pub use nott_format::backup::{BackupFormatError, BackupHeader, HeaderFields, Kdf, KeyFile};
use nott_format::backup::{
    AUTHENTICATION_KEY_LEN, CIPHERTEXT_LEN, DATA_KEY_LEN, HEADER_LINES, MAX_HEADER_BYTES,
    PUBLIC_KEY_LEN, SALT_LEN, WRAPPED_KEY_LEN,
};
use nott_format::time::Timestamp;
use nott_format::Sha256Digest;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use zeroize::Zeroizing;

const _: () = assert!(PUBLIC_KEY_LEN == kem::PUBLIC_KEY_LEN);
const _: () = assert!(CIPHERTEXT_LEN == kem::CIPHERTEXT_LEN);
const _: () = assert!(WRAPPED_KEY_LEN == DATA_KEY_LEN + TAG_LEN);

/// Plaintext bytes per chunk.
pub const CHUNK_LEN: usize = 64 * 1024;
/// Shortest recovery secret accepted, in bytes.
pub const MIN_SECRET_LEN: usize = 1;
/// Longest recovery secret accepted, in bytes.
pub const MAX_SECRET_LEN: usize = 1024;

/// Least Argon2id memory accepted: 64 MiB, as in RFC 9106's second
/// recommended option.
pub const MIN_MEMORY_KIB: u32 = 64 * 1024;
/// Most Argon2id memory accepted: 2 GiB, as in RFC 9106's first
/// recommended option.
pub const MAX_MEMORY_KIB: u32 = 2 * 1024 * 1024;
/// Most Argon2id passes accepted.
pub const MAX_ITERATIONS: u32 = 10;
/// Most Argon2id lanes accepted.
pub const MAX_LANES: u32 = 16;
/// Least Argon2id work accepted, memory times passes: that of RFC 9106's
/// second recommended option (64 MiB, three passes). The first option
/// (2 GiB, one pass) also meets it.
pub const MIN_WORK_KIB: u64 = 3 * 64 * 1024;

/// Parameters for new backup keys: 512 MiB, three passes, four lanes. A
/// proposal, to be tuned on Tier A devices; not measured on a phone.
pub const DEFAULT_PARAMS: Argon2idParams = Argon2idParams {
    memory_kib: 512 * 1024,
    iterations: 3,
    lanes: 4,
};

const RECOVERY_LABEL: &[u8] = b"NottOS-backup-recovery-v1";
const WRAP_LABEL: &[u8] = b"NottOS-backup-wrap-v1";
const PAYLOAD_LABEL: &[u8] = b"NottOS-backup-payload-v1";
const WRAP_NONCE: [u8; NONCE_LEN] = [0; NONCE_LEN];
const SEALED_CHUNK_LEN: usize = CHUNK_LEN + TAG_LEN;

/// Why a backup could not be made or read.
#[derive(Debug)]
pub enum BackupError {
    /// Reading or writing failed.
    Io(io::Error),
    /// The header or backup key is not in canonical form.
    Format(BackupFormatError),
    /// The recovery secret is empty or longer than [`MAX_SECRET_LEN`].
    SecretLength,
    /// The Argon2id parameters are outside the limits of this module.
    KdfParameters,
    /// Not enough memory for Argon2id.
    OutOfMemory,
    /// The operating system provided no randomness.
    Randomness,
    /// The recovery public key in a backup key is not a valid hybrid key.
    InvalidPublicKey,
    /// The recovery secret does not belong to this backup.
    WrongSecret,
    /// The header is not authentic: it was changed, or made without the
    /// backup key.
    Inauthentic,
    /// A chunk of the payload is not authentic.
    Chunk {
        /// Zero-based chunk number.
        index: u64,
    },
    /// The payload ends before its final chunk.
    Truncated,
    /// Data follows the final chunk.
    TrailingData,
    /// An empty final chunk follows other chunks; a canonical backup never
    /// has one.
    EmptyFinalChunk,
    /// The payload has more chunks than the nonce can count.
    TooLong,
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
            Self::SecretLength => write!(
                f,
                "the recovery secret must be {MIN_SECRET_LEN} to {MAX_SECRET_LEN} bytes long"
            ),
            Self::KdfParameters => f.write_str("Argon2id parameters outside the accepted limits"),
            Self::OutOfMemory => f.write_str("not enough memory for Argon2id"),
            Self::Randomness => f.write_str("the operating system provided no randomness"),
            Self::InvalidPublicKey => f.write_str("invalid recovery public key"),
            Self::WrongSecret => f.write_str("the recovery secret does not open this backup"),
            Self::Inauthentic => f.write_str("the backup header is not authentic"),
            Self::Chunk { index } => write!(f, "chunk {index} of the payload is not authentic"),
            Self::Truncated => f.write_str("the backup is cut short"),
            Self::TrailingData => f.write_str("data follows the end of the backup"),
            Self::EmptyFinalChunk => f.write_str("the backup ends with an empty chunk"),
            Self::TooLong => f.write_str("the backup is too long"),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<io::Error> for BackupError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<BackupFormatError> for BackupError {
    fn from(error: BackupFormatError) -> Self {
        Self::Format(error)
    }
}

/// Fails unless `kdf` is within the limits above. Checked before any key is
/// derived, so a hostile header cannot make the reader allocate more than
/// [`MAX_MEMORY_KIB`].
pub fn check_kdf(kdf: &Kdf) -> Result<(), BackupError> {
    let work = u64::from(kdf.memory_kib) * u64::from(kdf.iterations);
    let within = (MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&kdf.memory_kib)
        && (1..=MAX_ITERATIONS).contains(&kdf.iterations)
        && (1..=MAX_LANES).contains(&kdf.lanes)
        && work >= MIN_WORK_KIB;
    if within {
        Ok(())
    } else {
        Err(BackupError::KdfParameters)
    }
}

/// The keys derived from the recovery secret. Deriving them is slow on
/// purpose; derive once to open several backups made with one backup key.
pub struct RecoveryKeys {
    kdf: Kdf,
    secret_key: KemSecretKey,
    public_key: KemPublicKey,
    authentication_key: Zeroizing<[u8; AUTHENTICATION_KEY_LEN]>,
}

impl RecoveryKeys {
    /// Derives the keys from the recovery secret and the parameters and
    /// salt found in a backup header.
    pub fn derive(secret: &[u8], kdf: &Kdf) -> Result<Self, BackupError> {
        if !(MIN_SECRET_LEN..=MAX_SECRET_LEN).contains(&secret.len()) {
            return Err(BackupError::SecretLength);
        }
        check_kdf(kdf)?;
        let params = Argon2idParams {
            memory_kib: kdf.memory_kib,
            iterations: kdf.iterations,
            lanes: kdf.lanes,
        };
        let seed = password::argon2id(secret, &kdf.salt, params).map_err(|error| match error {
            KdfError::OutOfMemory => BackupError::OutOfMemory,
            KdfError::Parameters => BackupError::KdfParameters,
        })?;
        let mut expanded = Zeroizing::new([0u8; 128]);
        shake256(&[RECOVERY_LABEL, &seed[..]], &mut expanded[..]);
        let mut x25519 = Zeroizing::new([0u8; kem::X25519_LEN]);
        let mut ml_kem = Zeroizing::new([0u8; kem::ML_KEM_SEED_LEN]);
        let mut authentication_key = Zeroizing::new([0u8; AUTHENTICATION_KEY_LEN]);
        x25519.copy_from_slice(&expanded[..32]);
        ml_kem.copy_from_slice(&expanded[32..96]);
        authentication_key.copy_from_slice(&expanded[96..]);
        let secret_key = KemSecretKey::from_seeds(*x25519, *ml_kem);
        let public_key = secret_key.public_key();
        Ok(Self {
            kdf: *kdf,
            secret_key,
            public_key,
            authentication_key,
        })
    }
}

impl fmt::Debug for RecoveryKeys {
    /// Never prints key material.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecoveryKeys({})", key_id(&self.public_key))
    }
}

/// SHA-256 of a recovery public key.
fn key_id(public_key: &KemPublicKey) -> Sha256Digest {
    Sha256Digest::of_bytes(&public_key.to_bytes())
}

/// What the device keeps to make backups: the recovery public key and the
/// authentication key, never the recovery secret.
pub struct BackupKey {
    file: KeyFile,
    public_key: KemPublicKey,
}

impl BackupKey {
    /// Derives a backup key from a recovery secret, with a fresh random salt.
    /// The caller then forgets the secret: the device never needs it again.
    pub fn create(secret: &[u8], params: Argon2idParams) -> Result<Self, BackupError> {
        let salt = nott_crypto::random_bytes::<SALT_LEN>().map_err(|_| BackupError::Randomness)?;
        Self::derive(
            secret,
            Kdf {
                memory_kib: params.memory_kib,
                iterations: params.iterations,
                lanes: params.lanes,
                salt: *salt,
            },
        )
    }

    /// Derives the backup key for a recovery secret and known parameters.
    pub fn derive(secret: &[u8], kdf: Kdf) -> Result<Self, BackupError> {
        let keys = RecoveryKeys::derive(secret, &kdf)?;
        let mut public_key = Box::new([0u8; PUBLIC_KEY_LEN]);
        public_key.copy_from_slice(&keys.public_key.to_bytes());
        Ok(Self {
            file: KeyFile {
                kdf,
                public_key,
                authentication_key: keys.authentication_key,
            },
            public_key: keys.public_key,
        })
    }

    /// Parses a stored backup key and checks its public key.
    pub fn parse(text: &str) -> Result<Self, BackupError> {
        let file = KeyFile::parse(text)?;
        check_kdf(&file.kdf)?;
        let public_key = KemPublicKey::from_bytes(&file.public_key[..])
            .map_err(|_| BackupError::InvalidPublicKey)?;
        Ok(Self { file, public_key })
    }

    /// The canonical text form; secret, and wiped when dropped.
    pub fn to_text(&self) -> Zeroizing<String> {
        self.file.to_text()
    }

    /// The parameters and salt the key was derived with.
    pub fn kdf(&self) -> &Kdf {
        &self.file.kdf
    }

    /// SHA-256 of the recovery public key; also in every backup header.
    pub fn recovery_key_id(&self) -> Sha256Digest {
        key_id(&self.public_key)
    }
}

impl fmt::Debug for BackupKey {
    /// Never prints the authentication key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BackupKey({})", self.recovery_key_id())
    }
}

/// Caller-supplied randomness for [`encrypt_with_randomness`].
pub struct Randomness {
    /// The data key.
    pub data_key: [u8; DATA_KEY_LEN],
    /// The ephemeral X25519 secret of the encapsulation.
    pub x25519_ephemeral: [u8; kem::X25519_LEN],
    /// The ML-KEM-1024 encapsulation message `m`.
    pub ml_kem_m: [u8; 32],
}

/// Encrypts everything `input` yields into `output` as one backup. Returns
/// the number of plaintext bytes.
pub fn encrypt<R: Read, W: Write>(
    key: &BackupKey,
    created: Timestamp,
    input: R,
    output: W,
) -> Result<u64, BackupError> {
    let random = |_| BackupError::Randomness;
    let data_key = nott_crypto::random_bytes::<DATA_KEY_LEN>().map_err(random)?;
    let x25519 = nott_crypto::random_bytes::<{ kem::X25519_LEN }>().map_err(random)?;
    let ml_kem_m = nott_crypto::random_bytes::<32>().map_err(random)?;
    let randomness = Randomness {
        data_key: *data_key,
        x25519_ephemeral: *x25519,
        ml_kem_m: *ml_kem_m,
    };
    encrypt_with_randomness(key, created, randomness, input, output)
}

/// [`encrypt`] with caller-supplied randomness. For tests and reproduction
/// only: reusing randomness destroys security.
pub fn encrypt_with_randomness<R: Read, W: Write>(
    key: &BackupKey,
    created: Timestamp,
    randomness: Randomness,
    input: R,
    mut output: W,
) -> Result<u64, BackupError> {
    let data_key = Zeroizing::new(randomness.data_key);
    let (ciphertext, shared) = kem::encapsulate_with_randomness(
        &key.public_key,
        randomness.x25519_ephemeral,
        randomness.ml_kem_m,
    )
    .map_err(|_| BackupError::InvalidPublicKey)?;
    let mut kem_ciphertext = Box::new([0u8; CIPHERTEXT_LEN]);
    kem_ciphertext.copy_from_slice(&ciphertext.to_bytes());
    let fields = HeaderFields {
        created,
        kdf: key.file.kdf,
        recovery_key_id: key.recovery_key_id(),
        kem_ciphertext,
    };
    let wrap_key = sha3_256(&[WRAP_LABEL, &shared[..], &key.file.authentication_key[..]]);
    let wrapped = aead::seal(
        &wrap_key,
        &WRAP_NONCE,
        fields.authenticated_text().as_bytes(),
        &data_key[..],
    )
    .map_err(|_| BackupError::TooLong)?;
    let wrapped: [u8; WRAPPED_KEY_LEN] = wrapped.as_slice().try_into().expect("fixed length");
    output.write_all(BackupHeader::new(fields, wrapped).to_string().as_bytes())?;
    let payload_key = sha3_256(&[PAYLOAD_LABEL, &data_key[..]]);
    let mut chunks = Chunks::new(input);
    let mut buffer = Zeroizing::new(vec![0u8; CHUNK_LEN]);
    let mut index: u64 = 0;
    let mut total: u64 = 0;
    loop {
        let (len, last) = chunks.next(&mut buffer)?;
        let sealed = aead::seal(&payload_key, &chunk_nonce(index, last), b"", &buffer[..len])
            .map_err(|_| BackupError::TooLong)?;
        output.write_all(&sealed)?;
        total += len as u64;
        if last {
            output.flush()?;
            return Ok(total);
        }
        index = next_index(index)?;
    }
}

/// An opened backup whose header has been authenticated.
pub struct Decryptor<R> {
    reader: BufReader<R>,
    header: BackupHeader,
    payload_key: Zeroizing<[u8; 32]>,
}

impl<R: Read> Decryptor<R> {
    /// Reads the header, derives the recovery keys from `secret` and
    /// authenticates the header. Nothing of the payload is read yet.
    pub fn open(secret: &[u8], reader: R) -> Result<Self, BackupError> {
        let (reader, header) = Self::read(reader)?;
        let keys = RecoveryKeys::derive(secret, &header.fields().kdf)?;
        Self::authenticate(&keys, reader, header)
    }

    /// Like [`Decryptor::open`], with keys already derived.
    pub fn open_with(keys: &RecoveryKeys, reader: R) -> Result<Self, BackupError> {
        let (reader, header) = Self::read(reader)?;
        Self::authenticate(keys, reader, header)
    }

    fn read(reader: R) -> Result<(BufReader<R>, BackupHeader), BackupError> {
        let mut reader = BufReader::new(reader);
        let text = read_header(&mut reader)?;
        let header = BackupHeader::parse(&text)?;
        Ok((reader, header))
    }

    fn authenticate(
        keys: &RecoveryKeys,
        reader: BufReader<R>,
        header: BackupHeader,
    ) -> Result<Self, BackupError> {
        let fields = header.fields();
        if keys.kdf != fields.kdf || key_id(&keys.public_key) != fields.recovery_key_id {
            return Err(BackupError::WrongSecret);
        }
        let ciphertext = KemCiphertext::from_bytes(&fields.kem_ciphertext[..])
            .map_err(|_| BackupError::Inauthentic)?;
        let shared = keys
            .secret_key
            .decapsulate(&ciphertext)
            .map_err(|_: KemError| BackupError::Inauthentic)?;
        let wrap_key = sha3_256(&[WRAP_LABEL, &shared[..], &keys.authentication_key[..]]);
        let data_key = aead::open(
            &wrap_key,
            &WRAP_NONCE,
            fields.authenticated_text().as_bytes(),
            header.wrapped_key(),
        )
        .map_err(|_| BackupError::Inauthentic)?;
        let payload_key = sha3_256(&[PAYLOAD_LABEL, &data_key[..]]);
        Ok(Self {
            reader,
            header,
            payload_key,
        })
    }

    /// The authenticated header.
    pub fn header(&self) -> &BackupHeader {
        &self.header
    }

    /// When the backup was made, by the clock of the device that made it.
    pub fn created(&self) -> Timestamp {
        self.header.fields().created
    }

    /// Decrypts the payload into `output`. Returns the number of plaintext
    /// bytes. On error, whatever was written must be discarded.
    pub fn decrypt_to<W: Write>(self, mut output: W) -> Result<u64, BackupError> {
        let key = self.payload_key;
        let mut chunks = Chunks::new(self.reader);
        let mut buffer = vec![0u8; SEALED_CHUNK_LEN];
        let mut index: u64 = 0;
        let mut total: u64 = 0;
        loop {
            let (len, last) = chunks.next(&mut buffer)?;
            let sealed = &buffer[..len];
            let plaintext = aead::open(&key, &chunk_nonce(index, last), b"", sealed)
                .map_err(|_: AeadError| diagnose(&key, index, last, sealed))?;
            if last && plaintext.is_empty() && index > 0 {
                return Err(BackupError::EmptyFinalChunk);
            }
            output.write_all(&plaintext)?;
            total += plaintext.len() as u64;
            if last {
                output.flush()?;
                return Ok(total);
            }
            index = next_index(index)?;
        }
    }
}

/// Names the failure of a chunk that did not open: if it opens with the
/// other value of `last`, the stream was cut at a chunk boundary or had
/// data appended after its end.
fn diagnose(key: &[u8; 32], index: u64, last: bool, sealed: &[u8]) -> BackupError {
    if sealed.len() < TAG_LEN {
        return BackupError::Truncated;
    }
    if aead::open(key, &chunk_nonce(index, !last), b"", sealed).is_ok() {
        return if last {
            BackupError::Truncated
        } else {
            BackupError::TrailingData
        };
    }
    BackupError::Chunk { index }
}

fn chunk_nonce(index: u64, last: bool) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[3..11].copy_from_slice(&index.to_be_bytes());
    nonce[11] = u8::from(last);
    nonce
}

fn next_index(index: u64) -> Result<u64, BackupError> {
    index.checked_add(1).ok_or(BackupError::TooLong)
}

/// Reads exactly the header's lines, and not one byte of the payload. At
/// most one byte more than a header may hold is read; the parser then
/// reports the header as too large.
fn read_header<R: Read>(reader: &mut BufReader<R>) -> Result<String, BackupError> {
    let mut bytes = Vec::new();
    for _ in 0..HEADER_LINES {
        let room = (MAX_HEADER_BYTES + 1 - bytes.len()) as u64;
        reader.by_ref().take(room).read_until(b'\n', &mut bytes)?;
        if bytes.last() != Some(&b'\n') {
            break;
        }
    }
    // Bytes that are not UTF-8 become U+FFFD, which the parser reports as
    // a character outside printable ASCII, on the right line.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Cuts a stream into chunks, looking one byte ahead to know which chunk is
/// the last.
struct Chunks<R> {
    reader: R,
    pending: Option<u8>,
}

impl<R: Read> Chunks<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            pending: None,
        }
    }

    /// Fills `buffer` as far as the stream allows. Returns the length
    /// filled and whether the stream ends there.
    fn next(&mut self, buffer: &mut [u8]) -> io::Result<(usize, bool)> {
        let mut filled = 0;
        if let Some(byte) = self.pending.take() {
            buffer[0] = byte;
            filled = 1;
        }
        filled += read_full(&mut self.reader, &mut buffer[filled..])?;
        if filled < buffer.len() {
            return Ok((filled, true));
        }
        let mut one = [0u8; 1];
        if read_full(&mut self.reader, &mut one)? == 0 {
            return Ok((filled, true));
        }
        self.pending = Some(one[0]);
        Ok((filled, false))
    }
}

/// Reads until `buffer` is full or the stream ends.
fn read_full<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}
