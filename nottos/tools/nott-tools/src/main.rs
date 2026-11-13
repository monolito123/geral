// SPDX-License-Identifier: Apache-2.0

//! NottOS host tools.
//!
//! One binary with a subcommand per check, so that the build entry point,
//! CI and a person at a terminal all run exactly the same code.

mod args;
mod manifest_cmd;
mod repro;
mod sbom;
mod sign_cmd;
mod size;
mod storage;
mod tier_cmd;
mod util;

use std::process::ExitCode;

/// Result of a command that ran to completion.
pub(crate) enum Outcome {
    /// The command succeeded. The text goes to standard output.
    Pass(String),
    /// A check ran and found a problem. The text goes to standard output.
    Fail(String),
}

/// `Err` is a usage or I/O error: the check could not be carried out.
pub(crate) type CommandResult = Result<Outcome, String>;

const USAGE: &str = "\
usage: nott-tools <command>

  key generate --out-secret FILE --out-public FILE
  key id PUBLIC_KEY
  sign FILE --key SECRET_KEY --context NAME --out SIGNATURE
  verify FILE --public-key FILE --signature FILE --context NAME
  manifest create --version V --build-id ID --security-patch YYYY-MM-DD
                  --target T --vb-key FILE --image NAME=FILE... --out FILE
  manifest verify MANIFEST --dir DIR [--vb-key FILE] [--only NAME[,NAME...]]
                  [--public-key FILE --signature FILE]
  repro compare DIR_A DIR_B
  size check MANIFEST --budget-bytes N --images NAME[,NAME...]
  sbom --metadata FILE --lock FILE --created YYYY-MM-DDThh:mm:ssZ
       --name NAME --out FILE
  tier check DESCRIPTOR
  storage check REPORT [--exceptions FILE]

exit status: 0 success, 1 a check failed, 2 usage or I/O error
";

fn main() -> ExitCode {
    let mut raw = Vec::new();
    for argument in std::env::args_os().skip(1) {
        match argument.into_string() {
            Ok(text) => raw.push(text),
            Err(_) => {
                eprintln!("nott-tools: arguments must be valid UTF-8");
                return ExitCode::from(2);
            }
        }
    }
    let words: Vec<&str> = raw.iter().map(String::as_str).collect();
    match run(&words) {
        Ok(Outcome::Pass(report)) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Ok(Outcome::Fail(report)) => {
            print!("{report}");
            ExitCode::from(1)
        }
        Err(message) => {
            eprintln!("nott-tools: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(words: &[&str]) -> CommandResult {
    match words {
        ["key", "generate", rest @ ..] => sign_cmd::key_generate(rest),
        ["key", "id", rest @ ..] => sign_cmd::key_id(rest),
        ["sign", rest @ ..] => sign_cmd::sign(rest),
        ["verify", rest @ ..] => sign_cmd::verify(rest),
        ["manifest", "create", rest @ ..] => manifest_cmd::create(rest),
        ["manifest", "verify", rest @ ..] => manifest_cmd::verify(rest),
        ["repro", "compare", rest @ ..] => repro::compare(rest),
        ["size", "check", rest @ ..] => size::check(rest),
        ["sbom", rest @ ..] => sbom::run(rest),
        ["tier", "check", rest @ ..] => tier_cmd::check(rest),
        ["storage", "check", rest @ ..] => storage::check(rest),
        [] | ["help"] | ["--help"] => Ok(Outcome::Pass(USAGE.to_owned())),
        _ => Err(format!("unknown command\n{USAGE}")),
    }
}
