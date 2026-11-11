// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_assembly::character::ChfChange;

#[derive(Clone, Copy)]
pub(crate) enum TextField {
    Identical,
    Path(usize),
    Disposition(usize),
    Old(usize),
    New(usize),
}

pub(crate) struct TextSite {
    pub(crate) field: TextField,
    pub(crate) range: Range<u64>,
}

pub(crate) fn write<'change>(
    writer: &mut dyn Write,
    changes: impl IntoIterator<Item = &'change ChfChange>,
    observe: impl FnMut(TextSite) -> io::Result<()>,
) -> io::Result<()> {
    let mut out = TextWriter {
        writer,
        observe,
        bytes: 0,
    };
    let mut changes = changes.into_iter().peekable();
    if changes.peek().is_none() {
        out.datum(TextField::Identical, "no differences")?;
        out.text("\n")?;
    }
    for (index, change) in changes.enumerate() {
        out.change(index, change)?;
    }
    out.writer.flush()
}

struct TextWriter<'out, Observer> {
    writer: &'out mut dyn Write,
    observe: Observer,
    bytes: u64,
}

impl<Observer: FnMut(TextSite) -> io::Result<()>> TextWriter<'_, Observer> {
    fn change(&mut self, index: usize, change: &ChfChange) -> io::Result<()> {
        match (&change.old, &change.new) {
            (Some(old), Some(new)) => {
                self.path(index, &change.path)?;
                self.datum(TextField::Old(index), old)?;
                self.text(" -> ")?;
                self.datum(TextField::New(index), new)?;
                self.text("\n")
            }
            (Some(old), None) => {
                self.one_value(index, &change.path, "removed", TextField::Old(index), old)
            }
            (None, Some(new)) => {
                self.one_value(index, &change.path, "added", TextField::New(index), new)
            }
            (None, None) => Ok(()),
        }
    }

    fn one_value(
        &mut self,
        index: usize,
        path: &str,
        disposition: &str,
        field: TextField,
        value: &str,
    ) -> io::Result<()> {
        self.path(index, path)?;
        self.datum(TextField::Disposition(index), disposition)?;
        self.text(" (")?;
        self.datum(field, value)?;
        self.text(")\n")
    }

    fn path(&mut self, index: usize, path: &str) -> io::Result<()> {
        self.datum(TextField::Path(index), path)?;
        self.text(": ")
    }

    fn datum(&mut self, field: TextField, text: &str) -> io::Result<()> {
        let start = self.bytes;
        self.text(text)?;
        (self.observe)(TextSite {
            field,
            range: start..self.bytes,
        })
    }

    fn text(&mut self, text: &str) -> io::Result<()> {
        self.writer.write_all(text.as_bytes())?;
        self.bytes = self
            .bytes
            .checked_add(text.len() as u64)
            .ok_or_else(|| io::Error::other("CHF text byte count exceeds u64"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ChfChange, TextField, write};

    #[test]
    fn source_spans_count_utf8_bytes() -> std::io::Result<()> {
        let changes = [ChfChange {
            path: "Äugen".to_owned(),
            raw_path: "raw".to_owned(),
            old: Some("α".to_owned()),
            new: Some("β".to_owned()),
        }];
        let mut output = Vec::new();
        let mut fields = Vec::new();
        write(&mut output, &changes, |field| {
            fields.push(field);
            Ok(())
        })?;
        assert_eq!(
            output,
            "Äugen: α -> β\n".as_bytes(),
            "text bytes are preserved"
        );
        for field in fields {
            let expected = match field.field {
                TextField::Path(0) => "Äugen",
                TextField::Old(0) => "α",
                TextField::New(0) => "β",
                _ => panic!("unexpected text datum"),
            };
            let start = usize::try_from(field.range.start).map_err(std::io::Error::other)?;
            let end = usize::try_from(field.range.end).map_err(std::io::Error::other)?;
            assert_eq!(
                output.get(start..end),
                Some(expected.as_bytes()),
                "{expected}"
            );
        }
        Ok(())
    }
}
