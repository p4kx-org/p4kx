// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use serde::Serialize;
use serde_json::ser::{CharEscape, Formatter, PrettyFormatter, Serializer};

use crate::FieldPath;

pub(crate) fn write(value: &impl Serialize, mut output: impl Write) -> io::Result<Vec<FieldPath>> {
    let mut fields = Vec::new();
    let root = FieldPath::root();
    fields.push(root.clone());
    value.serialize(&mut Serializer::with_formatter(
        &mut output,
        FieldFormatter {
            pretty: PrettyFormatter::new(),
            path: root,
            fields: &mut fields,
            next_indices: Vec::new(),
            key: String::new(),
            recording_key: false,
        },
    ))?;
    output.write_all(b"\n")?;
    Ok(fields)
}

struct FieldFormatter<'output> {
    pretty: PrettyFormatter<'static>,
    path: FieldPath,
    fields: &'output mut Vec<FieldPath>,
    next_indices: Vec<usize>,
    key: String,
    recording_key: bool,
}

impl Formatter for FieldFormatter<'_> {
    fn begin_array<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.next_indices.push(0);
        self.pretty.begin_array(output)
    }

    fn end_array<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.next_indices.pop();
        self.pretty.end_array(output)
    }

    fn begin_array_value<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
        first: bool,
    ) -> io::Result<()> {
        let index = self
            .next_indices
            .last_mut()
            .ok_or_else(|| io::Error::other("array value has no array"))?;
        self.path.push_index(*index).map_err(io::Error::other)?;
        *index = index
            .checked_add(1)
            .ok_or_else(|| io::Error::other("JSON array index overflows"))?;
        self.fields.push(self.path.clone());
        self.pretty.begin_array_value(output, first)
    }

    fn end_array_value<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.path.pop();
        self.pretty.end_array_value(output)
    }

    fn begin_object<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.pretty.begin_object(output)
    }

    fn end_object<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.pretty.end_object(output)
    }

    fn begin_object_key<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.key.clear();
        self.recording_key = true;
        self.pretty.begin_object_key(output, first)
    }

    fn end_object_key<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.recording_key = false;
        self.pretty.end_object_key(output)
    }

    fn begin_object_value<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.path.push_field(&self.key).map_err(io::Error::other)?;
        self.fields.push(self.path.clone());
        self.pretty.begin_object_value(output)
    }

    fn end_object_value<W: Write + ?Sized>(&mut self, output: &mut W) -> io::Result<()> {
        self.path.pop();
        self.pretty.end_object_value(output)
    }

    fn write_string_fragment<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        if self.recording_key {
            self.key.push_str(fragment);
        }
        self.pretty.write_string_fragment(output, fragment)
    }

    fn write_char_escape<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
        escape: CharEscape,
    ) -> io::Result<()> {
        if self.recording_key {
            self.key.push(match escape {
                CharEscape::Quote => '"',
                CharEscape::ReverseSolidus => '\\',
                CharEscape::Solidus => '/',
                CharEscape::Backspace => '\x08',
                CharEscape::FormFeed => '\x0c',
                CharEscape::LineFeed => '\n',
                CharEscape::CarriageReturn => '\r',
                CharEscape::Tab => '\t',
                CharEscape::AsciiControl(byte) => char::from(byte),
            });
        }
        self.pretty.write_char_escape(output, escape)
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use serde_json::{Value, json};

    use super::write;

    #[test]
    fn output_matches_the_pretty_writer_and_every_value_is_addressed() -> io::Result<()> {
        let input = json!({
            "key/~\n\\\u{0001}": [{}, null, 42, [["array"], false], {"items": [12]}],
            "other": ["value", 21],
        });
        let mut emitted = Vec::new();
        let addresses = write(&input, &mut emitted)?;
        let mut original = serde_json::to_vec_pretty(&input)?;
        original.push(b'\n');
        assert_eq!(emitted, original, "observer uses identical JSON bytes");
        let decoded: Value = serde_json::from_slice(&emitted)?;
        for address in &addresses {
            assert!(decoded.pointer(address.as_str()).is_some(), "{address:?}");
        }
        assert_eq!(
            addresses.len(),
            15,
            "every container and leaf has an address"
        );
        Ok(())
    }
}
