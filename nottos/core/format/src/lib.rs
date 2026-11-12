//! Versioned on-disk formats for NottOS.
//!
//! Every format in this crate starts with a magic string and a format
//! version, so that algorithms and layouts can be replaced later without
//! ambiguity. Parsers accept exactly one byte representation per value: if a
//! document parses, re-serialising the result yields the same bytes. A
//! signature over the bytes therefore binds the content and nothing else.

pub mod digest;
pub mod manifest;

pub use digest::Sha256Digest;
pub use manifest::{BuildManifest, ImageEntry, ManifestError};
