//! The NottOS build manifest.
//!
//! The manifest states what a release consists of: version, build ID,
//! security patch level, the digest of the verified-boot key, and the size
//! and digest of every image. It is what release signatures cover and what
//! the Verify system app and the external verification tool compare a device
//! against.
//!
//! # Format, version 1
//!
//! Printable ASCII, one field per line, lines ended by a single line feed,
//! in exactly this order:
//!
//! ```text
//! nottos-build-manifest 1
//! version: 0.1.0
//! build-id: NOTT.20261112.001
//! security-patch: 2026-11-05
//! target: mini_x86_64
//! verified-boot-key-sha256: <64 hex digits>
//! image: <64 hex digits> <size in bytes> <name>
//! image: ...
//! ```
//!
//! Image lines are sorted by name and names are unique. Digests are SHA-256
//! in lower-case hexadecimal; format version 1 fixes that algorithm, and a
//! different algorithm means a new format version.
//!
//! The parser accepts only this canonical form. Any deviation (other line
//! endings, extra spaces, reordered or repeated lines, upper-case digits,
//! leading zeros) is an error, so two different byte strings never parse to
//! the same manifest.

use crate::digest::Sha256Digest;
use std::fmt;

/// First token of the first line.
pub const MAGIC: &str = "nottos-build-manifest";
/// The format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;
/// Upper bound on the size of a manifest in bytes.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// Upper bound on the number of images in a manifest.
pub const MAX_IMAGES: usize = 256;

const MAX_TOKEN_LEN: usize = 64;

const FIELD_VERSION: &str = "version: ";
const FIELD_BUILD_ID: &str = "build-id: ";
const FIELD_SECURITY_PATCH: &str = "security-patch: ";
const FIELD_TARGET: &str = "target: ";
const FIELD_VB_KEY: &str = "verified-boot-key-sha256: ";
const FIELD_IMAGE: &str = "image: ";

/// Why a manifest was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Longer than [`MAX_MANIFEST_BYTES`].
    TooLarge,
    /// A byte that is neither printable ASCII nor a line feed.
    InvalidCharacter {
        /// One-based line number.
        line: usize,
    },
    /// The text does not end with a line feed.
    MissingFinalNewline,
    /// The first line is not the manifest header.
    BadHeader,
    /// The header names a format version this crate does not implement.
    UnsupportedVersion(String),
    /// A line is missing or is not the field expected at that position.
    UnexpectedLine {
        /// One-based line number.
        line: usize,
        /// The field prefix that was expected there.
        expected: &'static str,
    },
    /// A field value is not in canonical form.
    BadValue {
        /// One-based line number, or `None` for a value passed to a
        /// constructor.
        line: Option<usize>,
        /// Which field.
        field: &'static str,
    },
    /// No image is listed.
    NoImages,
    /// More than [`MAX_IMAGES`] images.
    TooManyImages,
    /// Image names are not unique and strictly ascending.
    ImageOrder {
        /// One-based line number, or `None` for a value passed to a
        /// constructor.
        line: Option<usize>,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "manifest exceeds {MAX_MANIFEST_BYTES} bytes"),
            Self::InvalidCharacter { line } => {
                write!(f, "line {line}: character outside printable ASCII")
            }
            Self::MissingFinalNewline => f.write_str("manifest does not end with a line feed"),
            Self::BadHeader => write!(f, "first line is not `{MAGIC} <version>`"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported manifest format version {version}")
            }
            Self::UnexpectedLine { line, expected } => {
                write!(f, "line {line}: expected `{}`", expected.trim_end())
            }
            Self::BadValue {
                line: Some(line),
                field,
            } => write!(f, "line {line}: invalid {field}"),
            Self::BadValue { line: None, field } => write!(f, "invalid {field}"),
            Self::NoImages => f.write_str("manifest lists no image"),
            Self::TooManyImages => write!(f, "manifest lists more than {MAX_IMAGES} images"),
            Self::ImageOrder { line: Some(line) } => {
                write!(f, "line {line}: image names must be unique and ascending")
            }
            Self::ImageOrder { line: None } => f.write_str("image names must be unique"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// One image of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageEntry {
    name: String,
    size: u64,
    sha256: Sha256Digest,
}

impl ImageEntry {
    /// Creates an entry. `name` is 1 to 64 characters from `a-z 0-9 _ . -`
    /// and does not start with `.` or `-`.
    pub fn new(name: &str, size: u64, sha256: Sha256Digest) -> Result<Self, ManifestError> {
        if !is_image_name(name) {
            return Err(ManifestError::BadValue {
                line: None,
                field: "image name",
            });
        }
        Ok(Self {
            name: name.to_owned(),
            size,
            sha256,
        })
    }

    /// The image name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The image size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The SHA-256 digest of the image.
    pub fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
}

/// A validated build manifest. See the module documentation for the format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildManifest {
    version: String,
    build_id: String,
    security_patch: String,
    target: String,
    verified_boot_key_sha256: Sha256Digest,
    images: Vec<ImageEntry>,
}

impl BuildManifest {
    /// Creates a manifest. Images are sorted by name; duplicate names are an
    /// error.
    ///
    /// `version`, `build_id` and `target` are 1 to 64 characters from
    /// `A-Z a-z 0-9 . _ -`. `security_patch` is a `YYYY-MM-DD` date.
    pub fn new(
        version: &str,
        build_id: &str,
        security_patch: &str,
        target: &str,
        verified_boot_key_sha256: Sha256Digest,
        mut images: Vec<ImageEntry>,
    ) -> Result<Self, ManifestError> {
        check(is_token(version), None, "version")?;
        check(is_token(build_id), None, "build-id")?;
        check(is_date(security_patch), None, "security-patch")?;
        check(is_token(target), None, "target")?;
        if images.is_empty() {
            return Err(ManifestError::NoImages);
        }
        if images.len() > MAX_IMAGES {
            return Err(ManifestError::TooManyImages);
        }
        images.sort_by(|a, b| a.name.cmp(&b.name));
        if images.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(ManifestError::ImageOrder { line: None });
        }
        Ok(Self {
            version: version.to_owned(),
            build_id: build_id.to_owned(),
            security_patch: security_patch.to_owned(),
            target: target.to_owned(),
            verified_boot_key_sha256,
            images,
        })
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if text.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge);
        }
        let mut line = 1;
        for byte in text.bytes() {
            if byte == b'\n' {
                line += 1;
            } else if !(0x20..=0x7e).contains(&byte) {
                return Err(ManifestError::InvalidCharacter { line });
            }
        }
        let body = text
            .strip_suffix('\n')
            .ok_or(ManifestError::MissingFinalNewline)?;
        let mut lines = Lines::new(body);

        let header = lines.next_line().unwrap_or_default();
        let format = header
            .strip_prefix(MAGIC)
            .and_then(|rest| rest.strip_prefix(' '))
            .filter(|token| parse_decimal(token).is_some())
            .ok_or(ManifestError::BadHeader)?;
        if format != FORMAT_VERSION.to_string() {
            return Err(ManifestError::UnsupportedVersion(format.to_owned()));
        }

        let version = lines.field(FIELD_VERSION)?;
        check(is_token(version), lines.number(), "version")?;
        let build_id = lines.field(FIELD_BUILD_ID)?;
        check(is_token(build_id), lines.number(), "build-id")?;
        let security_patch = lines.field(FIELD_SECURITY_PATCH)?;
        check(is_date(security_patch), lines.number(), "security-patch")?;
        let target = lines.field(FIELD_TARGET)?;
        check(is_token(target), lines.number(), "target")?;
        let vb_key = lines.field(FIELD_VB_KEY)?;
        let verified_boot_key_sha256 =
            Sha256Digest::from_hex(vb_key).ok_or(ManifestError::BadValue {
                line: lines.number(),
                field: "verified-boot-key-sha256",
            })?;

        let mut images: Vec<ImageEntry> = Vec::new();
        while lines.has_more() {
            let value = lines.field(FIELD_IMAGE)?;
            let number = lines.number();
            if images.len() == MAX_IMAGES {
                return Err(ManifestError::TooManyImages);
            }
            let mut parts = value.split(' ');
            let (Some(digest), Some(size), Some(name), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(ManifestError::BadValue {
                    line: number,
                    field: "image line",
                });
            };
            let sha256 = Sha256Digest::from_hex(digest).ok_or(ManifestError::BadValue {
                line: number,
                field: "image digest",
            })?;
            let size = parse_decimal(size).ok_or(ManifestError::BadValue {
                line: number,
                field: "image size",
            })?;
            check(is_image_name(name), number, "image name")?;
            if images.last().is_some_and(|last| last.name.as_str() >= name) {
                return Err(ManifestError::ImageOrder { line: number });
            }
            images.push(ImageEntry {
                name: name.to_owned(),
                size,
                sha256,
            });
        }
        if images.is_empty() {
            return Err(ManifestError::NoImages);
        }

        Ok(Self {
            version: version.to_owned(),
            build_id: build_id.to_owned(),
            security_patch: security_patch.to_owned(),
            target: target.to_owned(),
            verified_boot_key_sha256,
            images,
        })
    }

    /// The operating-system version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The build identifier.
    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    /// The security patch level, `YYYY-MM-DD`.
    pub fn security_patch(&self) -> &str {
        &self.security_patch
    }

    /// The build target.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// The SHA-256 digest of the verified-boot public key.
    pub fn verified_boot_key_sha256(&self) -> Sha256Digest {
        self.verified_boot_key_sha256
    }

    /// All images, sorted by name.
    pub fn images(&self) -> &[ImageEntry] {
        &self.images
    }

    /// Looks an image up by name.
    pub fn image(&self, name: &str) -> Option<&ImageEntry> {
        self.images
            .binary_search_by(|entry| entry.name.as_str().cmp(name))
            .ok()
            .map(|index| &self.images[index])
    }
}

impl fmt::Display for BuildManifest {
    /// Writes the canonical text form, which [`BuildManifest::parse`] reads
    /// back to an equal value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_VERSION}{}", self.version)?;
        writeln!(f, "{FIELD_BUILD_ID}{}", self.build_id)?;
        writeln!(f, "{FIELD_SECURITY_PATCH}{}", self.security_patch)?;
        writeln!(f, "{FIELD_TARGET}{}", self.target)?;
        writeln!(f, "{FIELD_VB_KEY}{}", self.verified_boot_key_sha256)?;
        for image in &self.images {
            writeln!(
                f,
                "{FIELD_IMAGE}{} {} {}",
                image.sha256, image.size, image.name
            )?;
        }
        Ok(())
    }
}

/// Line cursor that remembers the one-based number of the last line taken.
struct Lines<'a> {
    inner: std::iter::Peekable<std::str::Split<'a, char>>,
    taken: usize,
}

impl<'a> Lines<'a> {
    fn new(body: &'a str) -> Self {
        Self {
            inner: body.split('\n').peekable(),
            taken: 0,
        }
    }

    fn next_line(&mut self) -> Option<&'a str> {
        let line = self.inner.next()?;
        self.taken += 1;
        Some(line)
    }

    fn has_more(&mut self) -> bool {
        self.inner.peek().is_some()
    }

    /// Number of the line most recently taken.
    fn number(&self) -> Option<usize> {
        Some(self.taken)
    }

    /// Takes the next line and strips the expected field prefix.
    fn field(&mut self, prefix: &'static str) -> Result<&'a str, ManifestError> {
        let unexpected = |line| ManifestError::UnexpectedLine {
            line,
            expected: prefix,
        };
        let line = self.next_line().ok_or(unexpected(self.taken + 1))?;
        line.strip_prefix(prefix).ok_or(unexpected(self.taken))
    }
}

fn check(valid: bool, line: Option<usize>, field: &'static str) -> Result<(), ManifestError> {
    if valid {
        Ok(())
    } else {
        Err(ManifestError::BadValue { line, field })
    }
}

fn is_token(text: &str) -> bool {
    (1..=MAX_TOKEN_LEN).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_image_name(text: &str) -> bool {
    let allowed = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_';
    (1..=MAX_TOKEN_LEN).contains(&text.len())
        && allowed(text.as_bytes()[0])
        && text
            .bytes()
            .all(|byte| allowed(byte) || matches!(byte, b'.' | b'-'))
}

fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let number = |range: std::ops::Range<usize>| parse_digits(&text[range]);
    matches!(
        (number(0..4), number(5..7), number(8..10)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

fn parse_digits(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Parses a decimal number with no sign and no leading zeros.
fn parse_decimal(text: &str) -> Option<u64> {
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    parse_digits(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const SAMPLE: &str = "\
nottos-build-manifest 1
version: 0.1.0
build-id: NOTT.20261112.001
security-patch: 2026-11-05
target: mini_x86_64
verified-boot-key-sha256: 1111111111111111111111111111111111111111111111111111111111111111
image: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad 3 boot.img
image: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 system.img
";

    fn sample() -> BuildManifest {
        BuildManifest::parse(SAMPLE).expect("sample parses")
    }

    /// Replaces the first occurrence of `from` in the sample.
    fn altered(from: &str, to: &str) -> String {
        assert!(SAMPLE.contains(from), "sample must contain {from:?}");
        SAMPLE.replacen(from, to, 1)
    }

    #[test]
    fn parses_sample() {
        let manifest = sample();
        assert_eq!(manifest.version(), "0.1.0");
        assert_eq!(manifest.build_id(), "NOTT.20261112.001");
        assert_eq!(manifest.security_patch(), "2026-11-05");
        assert_eq!(manifest.target(), "mini_x86_64");
        assert_eq!(manifest.verified_boot_key_sha256().to_hex(), KEY);
        assert_eq!(manifest.images().len(), 2);
        let boot = manifest.image("boot.img").expect("boot image listed");
        assert_eq!(boot.size(), 3);
        assert_eq!(boot.sha256(), Sha256Digest::of_bytes(b"abc"));
        assert_eq!(manifest.image("system.img").expect("listed").size(), 0);
        assert!(manifest.image("vendor.img").is_none());
    }

    #[test]
    fn serialises_to_the_same_bytes() {
        assert_eq!(sample().to_string(), SAMPLE);
    }

    #[test]
    fn constructor_sorts_and_matches_parser() {
        let images = vec![
            ImageEntry::new("system.img", 0, Sha256Digest::of_bytes(b"")).expect("valid"),
            ImageEntry::new("boot.img", 3, Sha256Digest::of_bytes(b"abc")).expect("valid"),
        ];
        let built = BuildManifest::new(
            "0.1.0",
            "NOTT.20261112.001",
            "2026-11-05",
            "mini_x86_64",
            Sha256Digest::from_hex(KEY).expect("valid"),
            images,
        )
        .expect("valid manifest");
        assert_eq!(built, sample());
        assert_eq!(built.to_string(), SAMPLE);
    }

    #[test]
    fn constructor_rejects_bad_input() {
        let digest = Sha256Digest::of_bytes(b"");
        let image = |name: &str| ImageEntry::new(name, 1, digest);
        let build = |version: &str, date: &str, images: Vec<ImageEntry>| {
            BuildManifest::new(version, "id", date, "target", digest, images)
        };
        let one = || vec![image("a").expect("valid")];

        assert!(build("1.0", "2026-01-31", one()).is_ok());
        assert_eq!(
            build("1 0", "2026-01-31", one()),
            Err(ManifestError::BadValue {
                line: None,
                field: "version"
            })
        );
        assert_eq!(
            build("1.0", "2026-13-01", one()),
            Err(ManifestError::BadValue {
                line: None,
                field: "security-patch"
            })
        );
        assert_eq!(
            build("1.0", "2026-01-31", vec![]),
            Err(ManifestError::NoImages)
        );
        assert_eq!(
            build(
                "1.0",
                "2026-01-31",
                vec![image("a").expect("valid"), image("a").expect("valid")]
            ),
            Err(ManifestError::ImageOrder { line: None })
        );
        for name in [
            "",
            ".hidden",
            "-dash",
            "Upper",
            "a b",
            "a/b",
            &"x".repeat(65),
        ] {
            assert!(image(name).is_err(), "name {name:?} must be rejected");
        }
    }

    #[test]
    fn rejects_non_canonical_text() {
        let cases: Vec<(String, ManifestError)> = vec![
            (
                SAMPLE.trim_end().to_owned(),
                ManifestError::MissingFinalNewline,
            ),
            (
                SAMPLE.replace('\n', "\r\n"),
                ManifestError::InvalidCharacter { line: 1 },
            ),
            (
                altered("0.1.0", "0.1.\u{e9}"),
                ManifestError::InvalidCharacter { line: 2 },
            ),
            (
                altered("nottos-build-manifest 1", "nottos-build-manifest"),
                ManifestError::BadHeader,
            ),
            (
                altered("nottos-build-manifest 1", "nottos-build-manifest 01"),
                ManifestError::BadHeader,
            ),
            (
                altered("nottos-build-manifest 1", "nottos-build-manifest  1"),
                ManifestError::BadHeader,
            ),
            (
                altered("nottos-build-manifest 1", "nottos-build-manifest 2"),
                ManifestError::UnsupportedVersion("2".to_owned()),
            ),
            (
                altered("version: 0.1.0", "version:  0.1.0"),
                ManifestError::BadValue {
                    line: Some(2),
                    field: "version",
                },
            ),
            (
                altered("version: 0.1.0", "version: 0.1.0 "),
                ManifestError::BadValue {
                    line: Some(2),
                    field: "version",
                },
            ),
            (
                altered("version: 0.1.0\n", ""),
                ManifestError::UnexpectedLine {
                    line: 2,
                    expected: FIELD_VERSION,
                },
            ),
            (
                altered("build-id: ", "Build-Id: "),
                ManifestError::UnexpectedLine {
                    line: 3,
                    expected: FIELD_BUILD_ID,
                },
            ),
            (
                altered("2026-11-05", "2026-11-5"),
                ManifestError::BadValue {
                    line: Some(4),
                    field: "security-patch",
                },
            ),
            (
                altered("2026-11-05", "2026-00-05"),
                ManifestError::BadValue {
                    line: Some(4),
                    field: "security-patch",
                },
            ),
            (
                altered("target: mini_x86_64\n", "target: mini_x86_64\n\n"),
                ManifestError::UnexpectedLine {
                    line: 6,
                    expected: FIELD_VB_KEY,
                },
            ),
            (
                altered(KEY, &KEY.replace('1', "A")),
                ManifestError::BadValue {
                    line: Some(6),
                    field: "verified-boot-key-sha256",
                },
            ),
            (
                altered(" 3 boot.img", " 03 boot.img"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image size",
                },
            ),
            (
                altered(" 3 boot.img", " +3 boot.img"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image size",
                },
            ),
            (
                altered(" 3 boot.img", " 18446744073709551616 boot.img"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image size",
                },
            ),
            (
                altered(" 3 boot.img", " 3 boot.img extra"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image line",
                },
            ),
            (
                altered(" 3 boot.img", " 3"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image line",
                },
            ),
            (
                altered(" 3 boot.img", " 3 Boot.img"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image name",
                },
            ),
            (
                altered("ba7816bf", "BA7816BF"),
                ManifestError::BadValue {
                    line: Some(7),
                    field: "image digest",
                },
            ),
            (
                altered(" 3 boot.img", " 3 system.img"),
                ManifestError::ImageOrder { line: Some(8) },
            ),
            (
                altered(" 3 boot.img", " 3 vendor.img"),
                ManifestError::ImageOrder { line: Some(8) },
            ),
            (
                altered(" 0 system.img\n", " 0 system.img\nnote: extra\n"),
                ManifestError::UnexpectedLine {
                    line: 9,
                    expected: FIELD_IMAGE,
                },
            ),
            (
                altered(" 0 system.img\n", " 0 system.img\n\n"),
                ManifestError::UnexpectedLine {
                    line: 9,
                    expected: FIELD_IMAGE,
                },
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(BuildManifest::parse(&text), Err(expected), "input:\n{text}");
        }
    }

    #[test]
    fn rejects_manifest_without_images() {
        let text: String = SAMPLE
            .lines()
            .filter(|line| !line.starts_with(FIELD_IMAGE))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(BuildManifest::parse(&text), Err(ManifestError::NoImages));
        assert_eq!(
            BuildManifest::parse(""),
            Err(ManifestError::MissingFinalNewline)
        );
        assert_eq!(BuildManifest::parse("\n"), Err(ManifestError::BadHeader));
    }

    #[test]
    fn enforces_size_limits() {
        let digest = Sha256Digest::of_bytes(b"x");
        let images = |count: usize| -> Vec<ImageEntry> {
            (0..count)
                .map(|index| {
                    let name = format!("{index:03}{}", "n".repeat(61));
                    ImageEntry::new(&name, u64::MAX, digest).expect("valid")
                })
                .collect()
        };
        let long = "T".repeat(64);
        let build = |images| BuildManifest::new(&long, &long, "2026-11-05", &long, digest, images);

        // The largest manifest the constructor allows fits the byte limit
        // and survives a round trip.
        let largest = build(images(MAX_IMAGES)).expect("valid");
        let text = largest.to_string();
        assert!(text.len() <= MAX_MANIFEST_BYTES);
        assert_eq!(BuildManifest::parse(&text), Ok(largest));

        assert_eq!(
            build(images(MAX_IMAGES + 1)),
            Err(ManifestError::TooManyImages)
        );
        let extra = format!("{text}image: {digest} 1 zzz\n");
        assert_eq!(
            BuildManifest::parse(&extra),
            Err(ManifestError::TooManyImages)
        );
        let padded = format!("{SAMPLE}{}", "x".repeat(MAX_MANIFEST_BYTES));
        assert_eq!(BuildManifest::parse(&padded), Err(ManifestError::TooLarge));
    }

    /// The property signatures rely on: no other byte string parses to the
    /// sample manifest. Checked for every single-bit change of the sample.
    #[test]
    fn every_bit_flip_is_rejected_or_changes_the_manifest() {
        let original = sample();
        let bytes = SAMPLE.as_bytes();
        for index in 0..bytes.len() {
            for bit in 0..8 {
                let mut mutated = bytes.to_vec();
                mutated[index] ^= 1 << bit;
                let Ok(text) = String::from_utf8(mutated) else {
                    continue;
                };
                if let Ok(manifest) = BuildManifest::parse(&text) {
                    assert_ne!(manifest, original, "byte {index}, bit {bit}");
                    assert_eq!(manifest.to_string(), text, "byte {index}, bit {bit}");
                }
            }
        }
    }

    /// Small deterministic generator, so the test needs no extra crate and
    /// fails reproducibly.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }

        fn text(&mut self, alphabet: &[u8], min: u64, max: u64) -> String {
            let length = min + self.below(max - min + 1);
            (0..length)
                .map(|_| alphabet[self.below(alphabet.len() as u64) as usize] as char)
                .collect()
        }
    }

    #[test]
    fn generated_manifests_round_trip() {
        const TOKEN: &[u8] = b"ABCXYZabcxyz0189._-";
        const NAME_START: &[u8] = b"abcxyz0189_";
        const NAME_REST: &[u8] = b"abcxyz0189_.-";
        let mut rng = Lcg(0x4e6f_7474_4f53);
        for _ in 0..300 {
            let count = 1 + rng.below(24);
            let mut names = std::collections::BTreeSet::new();
            while (names.len() as u64) < count {
                let name = rng.text(NAME_START, 1, 1) + &rng.text(NAME_REST, 0, 63);
                names.insert(name);
            }
            let images = names
                .iter()
                .map(|name| {
                    let digest = Sha256Digest::of_bytes(&rng.next().to_le_bytes());
                    let size = rng.next() << rng.below(32);
                    ImageEntry::new(name, size, digest).expect("generated name is valid")
                })
                .collect();
            let date = format!(
                "{:04}-{:02}-{:02}",
                rng.below(10_000),
                1 + rng.below(12),
                1 + rng.below(31)
            );
            let manifest = BuildManifest::new(
                &rng.text(TOKEN, 1, 64),
                &rng.text(TOKEN, 1, 64),
                &date,
                &rng.text(TOKEN, 1, 64),
                Sha256Digest::of_bytes(&rng.next().to_le_bytes()),
                images,
            )
            .expect("generated manifest is valid");
            let text = manifest.to_string();
            let parsed = BuildManifest::parse(&text).expect("canonical text parses");
            assert_eq!(parsed, manifest);
            assert_eq!(parsed.to_string(), text);
        }
    }

    #[test]
    fn errors_have_readable_messages() {
        let message = ManifestError::BadValue {
            line: Some(7),
            field: "image size",
        }
        .to_string();
        assert_eq!(message, "line 7: invalid image size");
        let message = ManifestError::UnexpectedLine {
            line: 2,
            expected: FIELD_VERSION,
        }
        .to_string();
        assert_eq!(message, "line 2: expected `version:`");
    }
}
