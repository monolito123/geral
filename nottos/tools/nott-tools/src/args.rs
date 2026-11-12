// SPDX-License-Identifier: Apache-2.0

//! Command-line parsing: `--name value` options and positional arguments.
//!
//! Deliberately small. Options always take a value, may repeat, and must be
//! on the command's allow-list. `--` ends option parsing.

use std::collections::BTreeMap;

pub(crate) struct Args<'a> {
    options: BTreeMap<&'a str, Vec<&'a str>>,
    positionals: Vec<&'a str>,
}

impl<'a> Args<'a> {
    pub(crate) fn parse(raw: &[&'a str], allowed: &[&str]) -> Result<Self, String> {
        let mut options: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut positionals = Vec::new();
        let mut words = raw.iter().copied();
        while let Some(word) = words.next() {
            if word == "--" {
                positionals.extend(words.by_ref());
            } else if let Some(name) = word.strip_prefix("--") {
                if !allowed.contains(&name) {
                    return Err(format!("unknown option --{name}"));
                }
                let value = words
                    .next()
                    .ok_or_else(|| format!("option --{name} needs a value"))?;
                options.entry(name).or_default().push(value);
            } else {
                positionals.push(word);
            }
        }
        Ok(Self {
            options,
            positionals,
        })
    }

    /// Every value given for a repeatable option, in order.
    pub(crate) fn repeated(&self, name: &str) -> &[&'a str] {
        self.options.get(name).map_or(&[], Vec::as_slice)
    }

    /// The value of an option that may be given at most once.
    pub(crate) fn optional(&self, name: &str) -> Result<Option<&'a str>, String> {
        match self.repeated(name) {
            [] => Ok(None),
            [value] => Ok(Some(value)),
            _ => Err(format!("option --{name} given more than once")),
        }
    }

    /// The value of an option that must be given exactly once.
    pub(crate) fn required(&self, name: &str) -> Result<&'a str, String> {
        self.optional(name)?
            .ok_or_else(|| format!("missing option --{name}"))
    }

    /// Exactly `N` positional arguments.
    pub(crate) fn positionals<const N: usize>(&self) -> Result<[&'a str; N], String> {
        <[&str; N]>::try_from(self.positionals.as_slice()).map_err(|_| {
            format!(
                "expected {N} positional argument(s), got {}",
                self.positionals.len()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_and_positionals() {
        let args = Args::parse(
            &["a", "--x", "1", "b", "--y", "2", "--x", "3"],
            &["x", "y", "z"],
        )
        .expect("valid");
        assert_eq!(args.positionals::<2>(), Ok(["a", "b"]));
        assert_eq!(args.repeated("x"), ["1", "3"]);
        assert_eq!(args.required("y"), Ok("2"));
        assert_eq!(args.optional("z"), Ok(None));
        assert!(args.repeated("z").is_empty());
    }

    #[test]
    fn double_dash_ends_options() {
        let args = Args::parse(&["--x", "1", "--", "--x", "-"], &["x"]).expect("valid");
        assert_eq!(args.positionals::<2>(), Ok(["--x", "-"]));
        assert_eq!(args.required("x"), Ok("1"));
    }

    #[test]
    fn errors() {
        let parse = |raw: &[&'static str]| Args::parse(raw, &["x"]).map(|_| ());
        assert_eq!(
            parse(&["--nope", "1"]),
            Err("unknown option --nope".to_owned())
        );
        assert_eq!(parse(&["--x"]), Err("option --x needs a value".to_owned()));

        let args = Args::parse(&["--x", "1", "--x", "2", "p"], &["x", "y"]).expect("valid");
        assert_eq!(
            args.required("x"),
            Err("option --x given more than once".to_owned())
        );
        assert_eq!(args.required("y"), Err("missing option --y".to_owned()));
        assert_eq!(
            args.positionals::<0>(),
            Err("expected 0 positional argument(s), got 1".to_owned())
        );
    }
}
