// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::argument_folder;
use crate::installer::argument_identity;
use crate::installer::binding::Binding;
use crate::installer::caller::Goal;
use crate::installer::declaration::Declaration;
use crate::installer::fault::Fault;

pub(crate) const MAX_ARGUMENT_WORDS: usize = 8192;
const MAX_ARGUMENTS: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Preflight,
    Prepare,
    Apply,
    Commit,
    Rollback,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) stage: Stage,
    pub(crate) binding: Binding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Reserve {
        product: u128,
        package: u128,
    },
    Seal {
        binding: Binding,
        declaration: Declaration,
    },
    Recover(Binding),
    Transaction(Invocation),
    Notify,
}

pub(crate) fn parse(input: impl IntoIterator<Item = Vec<u16>>) -> Result<Command, Fault> {
    let mut arguments = Vec::new();
    for argument in input {
        if arguments.len() == MAX_ARGUMENTS || argument.len() > MAX_ARGUMENT_WORDS {
            return Err(Fault::ArgumentLimit);
        }
        arguments.push(argument);
    }
    match arguments.as_slice() {
        [head] if word(head, "notify") => Ok(Command::Notify),
        [head, product, package] if word(head, "reserve") => Ok(Command::Reserve {
            product: argument_identity::code(product)?,
            package: argument_identity::code(package)?,
        }),
        [head, mode, product, package, captured] if word(head, "seal") && word(mode, "remove") => {
            seal(product, package, captured, None)
        }
        [head, mode, product, package, captured, predecessor, folder]
            if word(head, "seal") && word(mode, "ensure") =>
        {
            seal(product, package, captured, Some((predecessor, folder)))
        }
        [head, product, package, captured] => bound(head, product, package, captured),
        _ => Err(Fault::InvalidArguments),
    }
}

fn seal(
    product: &[u16],
    package: &[u16],
    captured: &[u16],
    ensure: Option<(&[u16], &[u16])>,
) -> Result<Command, Fault> {
    let (goal, predecessor) = match ensure {
        None => (Goal::Remove, None),
        Some((predecessor, folder)) => {
            let predecessor = if predecessor.is_empty() {
                None
            } else {
                Some(argument_identity::code(predecessor)?)
            };
            (Goal::Ensure(argument_folder::lease(folder)?), predecessor)
        }
    };
    let declaration = Declaration {
        product: argument_identity::code(product)?,
        package: argument_identity::code(package)?,
        predecessor,
        goal,
    };
    declaration.validate()?;
    Ok(Command::Seal {
        binding: captured_binding(product, package, captured)?,
        declaration,
    })
}

fn bound(
    head: &[u16],
    product: &[u16],
    package: &[u16],
    captured: &[u16],
) -> Result<Command, Fault> {
    let binding = captured_binding(product, package, captured)?;
    if word(head, "recover-reserved") {
        return Ok(Command::Recover(binding));
    }
    Ok(Command::Transaction(Invocation {
        stage: stage(head)?,
        binding,
    }))
}

fn captured_binding(product: &[u16], package: &[u16], captured: &[u16]) -> Result<Binding, Fault> {
    let binding = Binding::parse(captured)?;
    if binding.product != argument_identity::code(product)?
        || binding.package != argument_identity::code(package)?
    {
        return Err(Fault::InvalidIdentity);
    }
    Ok(binding)
}

fn stage(words: &[u16]) -> Result<Stage, Fault> {
    for (name, stage) in [
        ("preflight", Stage::Preflight),
        ("prepare", Stage::Prepare),
        ("apply", Stage::Apply),
        ("commit", Stage::Commit),
        ("rollback", Stage::Rollback),
        ("cancel-reserved", Stage::Cancel),
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
