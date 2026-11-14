// SPDX-License-Identifier: Apache-2.0

//! Verification of NottOS updates (`docs/adr/0009-update-system.md`).
//!
//! The design follows The Update Framework (TUF): a root of trust that
//! names the keys of each role and how many of them must sign; root
//! rotation in which each new root is signed by a threshold of both the old
//! and the new root keys; expiry against freeze attacks; monotonic sequence
//! numbers against rollback. It keeps two roles, root and release, because
//! one update document binds the whole release through the build manifest
//! digest.
//!
//! Everything here is pure: no file access, no clock. The caller supplies
//! the trusted state and the time, and persists the new state it gets back
//! once the update has been installed.

use nott_crypto::{context, verify, SignatureSet, Suite};
use nott_format::time::Timestamp;
use nott_format::update::{Role, Root, UpdateMetadata};
use nott_format::Sha256Digest;
use std::fmt;

/// The only suite root keys may use.
pub const ROOT_SUITE: Suite = Suite::Ed25519SlhDsaSha2_256s;
/// The only suite release keys may use.
pub const RELEASE_SUITE: Suite = Suite::Ed25519MlDsa87;

/// What a device remembers between updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustState {
    /// The current root.
    pub root: Root,
    /// Sequence number of the installed release.
    pub last_sequence: u64,
    /// No time earlier than this is believed: the latest `issued` time of
    /// any update metadata accepted so far.
    pub time_floor: Timestamp,
}

/// Why an update or a root was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// A document is not in its canonical format.
    Format(String),
    /// A role contains a key of a suite the role may not use.
    SuiteNotAllowed {
        /// `root` or `release`.
        role: &'static str,
    },
    /// Not enough distinct keys of the role produced a valid signature.
    ThresholdNotMet {
        /// Which keys: `root`, `new root` or `release`.
        role: &'static str,
        /// How many valid signatures were required.
        needed: usize,
        /// How many there were.
        valid: usize,
    },
    /// A new root does not have the next version number.
    RootVersion {
        /// The only acceptable version.
        expected: u64,
        /// The version offered.
        found: u64,
    },
    /// The current root has expired.
    RootExpired {
        /// When it expired.
        expires: Timestamp,
        /// The time used for the check.
        now: Timestamp,
    },
    /// The update metadata has expired.
    MetadataExpired {
        /// When it expired.
        expires: Timestamp,
        /// The time used for the check.
        now: Timestamp,
    },
    /// The update metadata is for another channel.
    WrongChannel {
        /// The device's channel.
        expected: String,
        /// The metadata's channel.
        found: String,
    },
    /// The update metadata is for another target.
    WrongTarget {
        /// The device's target.
        expected: String,
        /// The metadata's target.
        found: String,
    },
    /// The offered release is not newer than the installed one.
    NotNewer {
        /// Sequence number of the installed release.
        installed: u64,
        /// Sequence number offered.
        offered: u64,
    },
    /// The build manifest does not match the update metadata.
    ManifestMismatch,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(message) => write!(f, "malformed document: {message}"),
            Self::SuiteNotAllowed { role } => {
                write!(
                    f,
                    "the {role} role contains a key of a suite it may not use"
                )
            }
            Self::ThresholdNotMet {
                role,
                needed,
                valid,
            } => write!(
                f,
                "{valid} valid signature(s) by {role} keys, {needed} required"
            ),
            Self::RootVersion { expected, found } => {
                write!(
                    f,
                    "root version {found} offered, only {expected} can follow"
                )
            }
            Self::RootExpired { expires, now } => {
                write!(f, "the root expired at {expires} (now {now})")
            }
            Self::MetadataExpired { expires, now } => {
                write!(f, "the update metadata expired at {expires} (now {now})")
            }
            Self::WrongChannel { expected, found } => {
                write!(
                    f,
                    "metadata for channel `{found}`, device follows `{expected}`"
                )
            }
            Self::WrongTarget { expected, found } => {
                write!(f, "metadata for target `{found}`, device is `{expected}`")
            }
            Self::NotNewer { installed, offered } => write!(
                f,
                "release {offered} is not newer than installed release {installed}"
            ),
            Self::ManifestMismatch => {
                f.write_str("the build manifest does not match the update metadata")
            }
        }
    }
}

impl std::error::Error for UpdateError {}

/// Counts the distinct keys of `role` that made a valid signature over
/// `message` for `context`. Signatures by keys outside the role, invalid
/// signatures, and signatures for another purpose count for nothing.
pub fn valid_signatures(
    role: &Role,
    context: &str,
    message: &[u8],
    signatures: &SignatureSet,
) -> usize {
    // A signature set holds at most one signature per key.
    signatures
        .signatures()
        .iter()
        .filter(|signature| {
            role.key(signature.key_id())
                .is_some_and(|key| verify(key, context, message, signature).is_ok())
        })
        .count()
}

fn require(
    role: &Role,
    role_name: &'static str,
    context: &str,
    message: &[u8],
    signatures: &SignatureSet,
) -> Result<(), UpdateError> {
    let valid = valid_signatures(role, context, message, signatures);
    if valid < role.threshold() {
        return Err(UpdateError::ThresholdNotMet {
            role: role_name,
            needed: role.threshold(),
            valid,
        });
    }
    Ok(())
}

fn check_suites(root: &Root) -> Result<(), UpdateError> {
    if root
        .root_role()
        .keys()
        .iter()
        .any(|key| key.suite() != ROOT_SUITE)
    {
        return Err(UpdateError::SuiteNotAllowed { role: "root" });
    }
    if root
        .release_role()
        .keys()
        .iter()
        .any(|key| key.suite() != RELEASE_SUITE)
    {
        return Err(UpdateError::SuiteNotAllowed { role: "release" });
    }
    Ok(())
}

fn parse_root(text: &str) -> Result<Root, UpdateError> {
    let root = Root::parse(text).map_err(|error| UpdateError::Format(error.to_string()))?;
    check_suites(&root)?;
    Ok(root)
}

/// Checks the root that ships with the system. It is trusted because it is
/// on the verified system image, but it must still be signed by a threshold
/// of its own root keys, so that a root nobody can sign for is never
/// shipped.
pub fn check_initial_root(text: &str, signatures: &SignatureSet) -> Result<Root, UpdateError> {
    let root = parse_root(text)?;
    require(
        root.root_role(),
        "root",
        context::ROOT_METADATA,
        text.as_bytes(),
        signatures,
    )?;
    Ok(root)
}

/// Moves trust from `current` to the next root.
///
/// The new root must have the next version number and be signed by a
/// threshold of the current root keys and by a threshold of its own root
/// keys. Expiry is not checked here: an expired intermediate root may still
/// lead to a valid one. The final root's expiry is checked by
/// [`verify_update`].
pub fn rotate_root(
    current: &Root,
    text: &str,
    signatures: &SignatureSet,
) -> Result<Root, UpdateError> {
    let next = parse_root(text)?;
    let expected = current
        .version()
        .checked_add(1)
        .ok_or(UpdateError::RootVersion {
            expected: u64::MAX,
            found: next.version(),
        })?;
    if next.version() != expected {
        return Err(UpdateError::RootVersion {
            expected,
            found: next.version(),
        });
    }
    let message = text.as_bytes();
    require(
        current.root_role(),
        "root",
        context::ROOT_METADATA,
        message,
        signatures,
    )?;
    require(
        next.root_role(),
        "new root",
        context::ROOT_METADATA,
        message,
        signatures,
    )?;
    Ok(next)
}

/// What the device is.
#[derive(Debug, Clone, Copy)]
pub struct Device<'a> {
    /// The release channel it follows.
    pub channel: &'a str,
    /// Its build target.
    pub target: &'a str,
}

/// Checks update metadata against the trusted state.
///
/// `now` is the device clock. It is not trusted below the state's time
/// floor: a clock set back cannot make expired metadata fresh again.
/// Signatures are checked before the metadata is interpreted.
///
/// On success, returns the metadata and the state to persist once the
/// update has been installed.
pub fn verify_update(
    state: &TrustState,
    device: Device<'_>,
    text: &str,
    signatures: &SignatureSet,
    now: Timestamp,
) -> Result<(UpdateMetadata, TrustState), UpdateError> {
    let now = now.max(state.time_floor);
    if now >= state.root.expires() {
        return Err(UpdateError::RootExpired {
            expires: state.root.expires(),
            now,
        });
    }
    require(
        state.root.release_role(),
        "release",
        context::UPDATE_METADATA,
        text.as_bytes(),
        signatures,
    )?;
    let metadata =
        UpdateMetadata::parse(text).map_err(|error| UpdateError::Format(error.to_string()))?;
    if metadata.channel() != device.channel {
        return Err(UpdateError::WrongChannel {
            expected: device.channel.to_owned(),
            found: metadata.channel().to_owned(),
        });
    }
    if metadata.target() != device.target {
        return Err(UpdateError::WrongTarget {
            expected: device.target.to_owned(),
            found: metadata.target().to_owned(),
        });
    }
    if now >= metadata.expires() {
        return Err(UpdateError::MetadataExpired {
            expires: metadata.expires(),
            now,
        });
    }
    if metadata.sequence() <= state.last_sequence {
        return Err(UpdateError::NotNewer {
            installed: state.last_sequence,
            offered: metadata.sequence(),
        });
    }
    let next = TrustState {
        root: state.root.clone(),
        last_sequence: metadata.sequence(),
        time_floor: state.time_floor.max(metadata.issued()),
    };
    Ok((metadata, next))
}

/// Checks that `manifest` is the build manifest the metadata names.
///
/// The digest decides. The metadata's `manifest-size` is for the code that
/// downloads the manifest: it must stop reading at that many bytes, so that
/// a server cannot feed it endless data.
pub fn check_manifest(metadata: &UpdateMetadata, manifest: &[u8]) -> Result<(), UpdateError> {
    if Sha256Digest::of_bytes(manifest) == metadata.manifest_sha256() {
        Ok(())
    } else {
        Err(UpdateError::ManifestMismatch)
    }
}
