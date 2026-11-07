// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::argument_folder;
use crate::installer::argument_identity;
use crate::installer::caller::{Caller, Goal};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

pub(crate) const MAX_ARGUMENT_WORDS: usize = 8192;
const MAX_ARGUMENTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Preflight,
    Prepare,
    Apply,
    Commit,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) stage: Stage,
    pub(crate) expected: u64,
    pub(crate) caller: Caller,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Transaction(Invocation),
    Notify,
}

enum RawGoal<'a> {
    Remove,
    Ensure {
        predecessor: &'a [u16],
        folder: &'a [u16],
    },
}

struct Raw<'a> {
    head: &'a [u16],
    expected: &'a [u16],
    product: &'a [u16],
    package: &'a [u16],
    nonce: &'a [u16],
    goal: RawGoal<'a>,
}

pub(crate) fn parse(input: impl IntoIterator<Item = Vec<u16>>) -> Result<Command, Fault> {
    let mut arguments = Vec::new();
    for argument in input {
        if arguments.len() == MAX_ARGUMENTS || argument.len() > MAX_ARGUMENT_WORDS {
            return Err(Fault::ArgumentLimit);
        }
        arguments.push(argument);
    }
    if let [verb] = arguments.as_slice()
        && word(verb, "notify")
    {
        return Ok(Command::Notify);
    }
    decode(&raw(&arguments)?)
}

fn raw(arguments: &[Vec<u16>]) -> Result<Raw<'_>, Fault> {
    match arguments {
        [head, mode, expected, product, package, nonce] if word(mode, "remove") => Ok(Raw {
            head,
            expected,
            product,
            package,
            nonce,
            goal: RawGoal::Remove,
        }),
        [
            head,
            mode,
            expected,
            product,
            package,
            nonce,
            predecessor,
            folder,
        ] if word(mode, "ensure") => Ok(Raw {
            head,
            expected,
            product,
            package,
            nonce,
            goal: RawGoal::Ensure {
                predecessor,
                folder,
            },
        }),
        _ => Err(Fault::InvalidArguments),
    }
}

fn decode(raw: &Raw<'_>) -> Result<Command, Fault> {
    let (goal, predecessor) = match &raw.goal {
        RawGoal::Remove => (Goal::Remove, None),
        RawGoal::Ensure {
            predecessor,
            folder,
        } => {
            let previous = if predecessor.is_empty() {
                None
            } else {
                Some(argument_identity::code(predecessor)?)
            };
            (Goal::Ensure(argument_folder::lease(folder)?), previous)
        }
    };
    let caller = Caller {
        product: argument_identity::code(raw.product)?,
        package: argument_identity::code(raw.package)?,
        nonce: InvocationNonce::parse(raw.nonce)?,
        predecessor,
        goal,
    };
    caller.validate()?;
    Ok(Command::Transaction(Invocation {
        stage: stage(raw.head)?,
        expected: argument_identity::sequence(raw.expected)?,
        caller,
    }))
}

fn stage(words: &[u16]) -> Result<Stage, Fault> {
    for (name, stage) in [
        ("preflight", Stage::Preflight),
        ("prepare", Stage::Prepare),
        ("apply", Stage::Apply),
        ("commit", Stage::Commit),
        ("rollback", Stage::Rollback),
    ] {
        if word(words, name) {
            return Ok(stage);
        }
    }
    Err(Fault::InvalidArguments)
}

fn word(words: &[u16], expected: &str) -> bool {
    words.iter().copied().eq(expected.encode_utf16())
}

#[cfg(test)]
mod tests;
