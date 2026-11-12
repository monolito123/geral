// SPDX-License-Identifier: Apache-2.0

//! Small file helpers shared by the commands.

use std::fs;
use std::path::Path;

/// Reads a UTF-8 text file, refusing files larger than `limit` bytes.
pub(crate) fn read_text(path: &Path, limit: u64) -> Result<String, String> {
    let describe = |error: std::io::Error| format!("{}: {error}", path.display());
    let length = fs::metadata(path).map_err(describe)?.len();
    if length > limit {
        return Err(format!("{}: larger than {limit} bytes", path.display()));
    }
    fs::read_to_string(path).map_err(describe)
}

/// Writes a file, replacing any existing one.
pub(crate) fn write_text(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}
