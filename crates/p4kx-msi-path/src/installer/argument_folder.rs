// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::lease::Lease;

pub(crate) fn lease(words: &[u16]) -> Result<Lease, Fault> {
    let Some((&last, path)) = words.split_last() else {
        return Err(Fault::InvalidFolder);
    };
    if last != u16::from(b'.') || !path.last().is_some_and(|word| separator(*word)) {
        return Err(Fault::InvalidFolder);
    }
    let relative = absolute_tail(path)?;
    let mut normal = 0;
    let components = relative.split(|word| separator(*word));
    for component in components {
        if component.is_empty() {
            continue;
        }
        if component == [u16::from(b'.')]
            || component == [u16::from(b'.'), u16::from(b'.')]
            || component
                .iter()
                .any(|word| [0, 34, 42, 58, 63].contains(word))
        {
            return Err(Fault::InvalidFolder);
        }
        normal += 1;
    }
    if normal == 0 {
        return Err(Fault::InvalidFolder);
    }
    Lease::try_new(path.iter().copied().flat_map(u16::to_le_bytes).collect())
}

fn absolute_tail(words: &[u16]) -> Result<&[u16], Fault> {
    match words {
        [drive, colon, slash, tail @ ..]
            if ascii_letter(*drive) && *colon == u16::from(b':') && separator(*slash) =>
        {
            Ok(tail)
        }
        [left, right, tail @ ..] if separator(*left) && separator(*right) => {
            let mut components = tail.split(|word| separator(*word));
            let server = components.next().ok_or(Fault::InvalidFolder)?;
            let share = components.next().ok_or(Fault::InvalidFolder)?;
            if server.is_empty()
                || share.is_empty()
                || server.contains(&u16::from(b'?'))
                || server == [u16::from(b'.')]
            {
                return Err(Fault::InvalidFolder);
            }
            Ok(tail)
        }
        _ => Err(Fault::InvalidFolder),
    }
}

fn separator(word: u16) -> bool {
    word == u16::from(b'\\') || word == u16::from(b'/')
}
fn ascii_letter(word: u16) -> bool {
    (u16::from(b'a')..=u16::from(b'z')).contains(&word)
        || (u16::from(b'A')..=u16::from(b'Z')).contains(&word)
}
