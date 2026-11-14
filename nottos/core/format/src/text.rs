// SPDX-License-Identifier: Apache-2.0

//! Shared machinery for NottOS's canonical text formats.
//!
//! Every such format is printable ASCII, one field per line, each line ended
//! by a single line feed, and starts with a header line `<magic> <version>`.
//! Fields come in a fixed order and each has a fixed prefix.

/// A problem found before any field is interpreted, or a field that is not
/// where it must be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Longer than the format allows.
    TooLarge,
    /// A byte that is neither printable ASCII nor a line feed.
    InvalidCharacter {
        /// One-based line number.
        line: usize,
    },
    /// The text does not end with a line feed.
    MissingFinalNewline,
    /// The first line is not `<magic> <canonical decimal>`.
    BadHeader,
    /// The header names another format version.
    UnsupportedVersion(String),
    /// A line is missing or does not start with the expected prefix.
    UnexpectedLine {
        /// One-based line number.
        line: usize,
        /// The prefix that was expected there.
        expected: &'static str,
    },
}

/// Line cursor that remembers the one-based number of the last line taken.
pub struct Lines<'a> {
    inner: std::iter::Peekable<std::str::Split<'a, char>>,
    taken: usize,
}

impl<'a> Lines<'a> {
    /// Checks size, character set, final line feed and header, and returns a
    /// cursor positioned after the header.
    pub fn open(
        text: &'a str,
        max_bytes: usize,
        magic: &str,
        version: u32,
    ) -> Result<Self, Problem> {
        if text.len() > max_bytes {
            return Err(Problem::TooLarge);
        }
        let mut line = 1;
        for byte in text.bytes() {
            if byte == b'\n' {
                line += 1;
            } else if !(0x20..=0x7e).contains(&byte) {
                return Err(Problem::InvalidCharacter { line });
            }
        }
        let body = text
            .strip_suffix('\n')
            .ok_or(Problem::MissingFinalNewline)?;
        let mut lines = Self {
            inner: body.split('\n').peekable(),
            taken: 0,
        };
        let header = lines.next_line().unwrap_or_default();
        let found = header
            .strip_prefix(magic)
            .and_then(|rest| rest.strip_prefix(' '))
            .filter(|token| parse_decimal(token).is_some())
            .ok_or(Problem::BadHeader)?;
        if found != version.to_string() {
            return Err(Problem::UnsupportedVersion(found.to_owned()));
        }
        Ok(lines)
    }

    fn next_line(&mut self) -> Option<&'a str> {
        let line = self.inner.next()?;
        self.taken += 1;
        Some(line)
    }

    /// Whether any line is left.
    pub fn has_more(&mut self) -> bool {
        self.inner.peek().is_some()
    }

    /// Whether the next line starts with `prefix`, without taking it.
    pub fn peek_starts_with(&mut self, prefix: &str) -> bool {
        self.inner
            .peek()
            .is_some_and(|line| line.starts_with(prefix))
    }

    /// Number of the line most recently taken.
    pub fn number(&self) -> usize {
        self.taken
    }

    /// Takes the next line and strips the expected field prefix.
    pub fn field(&mut self, prefix: &'static str) -> Result<&'a str, Problem> {
        let unexpected = |line| Problem::UnexpectedLine {
            line,
            expected: prefix,
        };
        let line = self.next_line().ok_or(unexpected(self.taken + 1))?;
        line.strip_prefix(prefix).ok_or(unexpected(self.taken))
    }

    /// Fails if any line is left. `expected` names what should have been
    /// there instead: nothing.
    pub fn finish(mut self) -> Result<(), Problem> {
        if self.has_more() {
            return Err(Problem::UnexpectedLine {
                line: self.taken + 1,
                expected: "end of file",
            });
        }
        Ok(())
    }
}

/// Parses a decimal number with no sign and no leading zeros.
pub fn parse_decimal(text: &str) -> Option<u64> {
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    parse_digits(text)
}

/// Parses a run of ASCII digits, leading zeros allowed.
pub fn parse_digits(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_checks_envelope_and_header() {
        let open = |text| Lines::open(text, 64, "magic", 3).map(|_| ());
        assert_eq!(open("magic 3\nfield: x\n"), Ok(()));
        assert_eq!(open("magic 3\n"), Ok(()));
        assert_eq!(open("magic 3"), Err(Problem::MissingFinalNewline));
        assert_eq!(open(""), Err(Problem::MissingFinalNewline));
        assert_eq!(open("\n"), Err(Problem::BadHeader));
        assert_eq!(open("magic  3\n"), Err(Problem::BadHeader));
        assert_eq!(open("magic 03\n"), Err(Problem::BadHeader));
        assert_eq!(open("other 3\n"), Err(Problem::BadHeader));
        assert_eq!(
            open("magic 4\n"),
            Err(Problem::UnsupportedVersion("4".to_owned()))
        );
        assert_eq!(
            open("magic 3\r\n"),
            Err(Problem::InvalidCharacter { line: 1 })
        );
        assert_eq!(
            open("magic 3\n\tx\n"),
            Err(Problem::InvalidCharacter { line: 2 })
        );
        assert_eq!(open(&"x".repeat(65)), Err(Problem::TooLarge));
    }

    #[test]
    fn fields_in_order_then_finish() {
        let mut lines = Lines::open("m 1\na: 1\nb: 2\n", 64, "m", 1).expect("valid");
        assert_eq!(lines.field("a: "), Ok("1"));
        assert_eq!(lines.number(), 2);
        assert_eq!(
            lines.field("c: "),
            Err(Problem::UnexpectedLine {
                line: 3,
                expected: "c: "
            })
        );

        let mut lines = Lines::open("m 1\na: 1\n", 64, "m", 1).expect("valid");
        assert_eq!(lines.field("a: "), Ok("1"));
        assert_eq!(
            lines.field("b: "),
            Err(Problem::UnexpectedLine {
                line: 3,
                expected: "b: "
            })
        );

        let lines = Lines::open("m 1\nextra\n", 64, "m", 1).expect("valid");
        assert_eq!(
            lines.finish(),
            Err(Problem::UnexpectedLine {
                line: 2,
                expected: "end of file"
            })
        );
        assert_eq!(
            Lines::open("m 1\n", 64, "m", 1).expect("valid").finish(),
            Ok(())
        );
    }

    #[test]
    fn decimals() {
        assert_eq!(parse_decimal("0"), Some(0));
        assert_eq!(parse_decimal("18446744073709551615"), Some(u64::MAX));
        for bad in [
            "",
            "00",
            "01",
            "+1",
            "-1",
            "1 ",
            "18446744073709551616",
            "1e3",
        ] {
            assert_eq!(parse_decimal(bad), None, "{bad:?}");
        }
        assert_eq!(parse_digits("007"), Some(7));
    }
}
