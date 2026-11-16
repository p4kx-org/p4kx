// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufRead};

use super::profile::{ReferenceSets, Report, Section, increment, invalid, start_inventory};

const GROUPS: &[&str] = &[
    "exportHealth",
    "definitions",
    "configurations",
    "declarations",
    "placements",
    "projections",
    "provenance",
    "texts",
];
const SOURCE_REFS: &[u8] = b"sourceUsesRefs";

#[derive(Default)]
pub(super) struct Scanner {
    stack: Vec<Frame>,
    text: Option<Quoted>,
    bytes: u64,
    root_closed: bool,
    sections: Vec<Section>,
    source_rows: Option<u64>,
    use_rows: Option<u64>,
    references: ReferenceSets,
    fingerprint_references: bool,
}

enum Frame {
    Object(Object),
    Array(Array),
}

struct Object {
    group: Option<Group>,
    member: Option<Member>,
    expect_key: bool,
    reference_key: bool,
}

enum Group {
    Top,
    Nested(String),
}

struct Member {
    name: String,
    key_at: u64,
    value_at: Option<u64>,
}

#[derive(Clone, Copy)]
enum Array {
    Other,
    Sources,
    Uses,
    References,
}

struct Quoted {
    escaped: bool,
    key: Option<Key>,
}

struct Key {
    start: u64,
    label: Option<Vec<u8>>,
    reference_bytes: Option<usize>,
}

#[derive(Clone, Copy)]
enum ReferenceBoundary {
    None,
    Begin,
    End,
}

pub(super) fn read(mut input: impl BufRead) -> io::Result<Report> {
    let mut scanner = Scanner {
        fingerprint_references: true,
        ..Scanner::default()
    };
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return scanner.finish();
        }
        scanner.push(chunk)?;
        let len = chunk.len();
        input.consume(len);
    }
}

impl Scanner {
    pub(super) fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut span = self.references.active().then_some(0);
        let after = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("byte count overflow"))?;
        for (index, (at, byte)) in (self.bytes..after).zip(bytes.iter().copied()).enumerate() {
            match self.step(byte, at)? {
                ReferenceBoundary::None => {}
                ReferenceBoundary::Begin => span = Some(index),
                ReferenceBoundary::End => {
                    let start = span
                        .take()
                        .ok_or_else(|| invalid("missing reference array start"))?;
                    self.references.hash(
                        bytes
                            .get(start..=index)
                            .ok_or_else(|| invalid("reference array byte range"))?,
                    )?;
                    self.references.end(at + 1)?;
                }
            }
        }
        if let Some(start) = span {
            self.references.hash(
                bytes
                    .get(start..)
                    .ok_or_else(|| invalid("reference array remaining byte range"))?,
            )?;
        }
        self.bytes = after;
        Ok(())
    }

    fn step(&mut self, byte: u8, at: u64) -> io::Result<ReferenceBoundary> {
        if let Some(text) = self.text.take() {
            self.step_quoted(text, byte)?;
            return Ok(ReferenceBoundary::None);
        }
        if self.root_closed {
            return if byte.is_ascii_whitespace() {
                Ok(ReferenceBoundary::None)
            } else {
                Err(invalid("trailing catalog data"))
            };
        }
        if self.stack.is_empty() && (at != 0 || byte != b'{') {
            return Err(invalid("catalog is not one compact object"));
        }
        match byte {
            b'"' => self.open_quote(at)?,
            b'{' => self.open_object()?,
            b'}' => self.close_object(at)?,
            b'[' => {
                if self.open_array(at)? {
                    return Ok(ReferenceBoundary::Begin);
                }
            }
            b']' => match self.stack.pop() {
                Some(Frame::Array(Array::References)) => return Ok(ReferenceBoundary::End),
                Some(Frame::Array(_)) => {}
                _ => return Err(invalid("mismatched catalog array")),
            },
            b':' => {
                if let Some(Frame::Object(object)) = self.stack.last_mut()
                    && let Some(member) = &mut object.member
                    && member.value_at.replace(at + 1).is_some()
                {
                    return Err(invalid("duplicate section separator"));
                }
            }
            b',' => {
                if matches!(self.stack.last(), Some(Frame::Object(_))) {
                    self.end_member(at)?;
                    let Some(Frame::Object(object)) = self.stack.last_mut() else {
                        return Err(invalid("missing object after section member"));
                    };
                    object.expect_key = true;
                    object.reference_key = false;
                }
            }
            _ => {}
        }
        Ok(ReferenceBoundary::None)
    }

    fn step_quoted(&mut self, mut text: Quoted, byte: u8) -> io::Result<()> {
        if text.escaped {
            text.escaped = false;
        } else if byte == b'"' {
            if let Some(key) = text.key {
                self.closed_key(key)?;
            }
            return Ok(());
        } else if byte == b'\\' {
            text.escaped = true;
            if let Some(key) = &mut text.key {
                if key.label.is_some() {
                    return Err(invalid("escaped section name"));
                }
                key.reference_bytes = None;
            }
        } else if let Some(key) = &mut text.key {
            key.push(byte)?;
        }
        self.text = Some(text);
        Ok(())
    }

    fn open_quote(&mut self, at: u64) -> io::Result<()> {
        let key = match self.stack.last_mut() {
            Some(Frame::Object(object)) if object.expect_key => {
                object.expect_key = false;
                Some(Key {
                    start: at,
                    label: object.group.as_ref().map(|_| Vec::new()),
                    reference_bytes: self.fingerprint_references.then_some(0),
                })
            }
            Some(Frame::Array(Array::References)) => {
                self.references.item()?;
                None
            }
            _ => None,
        };
        self.text = Some(Quoted {
            escaped: false,
            key,
        });
        Ok(())
    }

    fn closed_key(&mut self, key: Key) -> io::Result<()> {
        let Some(Frame::Object(object)) = self.stack.last_mut() else {
            return Err(invalid("catalog field name outside object"));
        };
        object.reference_key = key.reference_bytes == Some(SOURCE_REFS.len());
        if let Some(label) = key.label {
            if label.is_empty() || object.member.is_some() {
                return Err(invalid("invalid schema section member"));
            }
            let name = String::from_utf8(label).map_err(io::Error::other)?;
            object.member = Some(Member {
                name,
                key_at: key.start,
                value_at: None,
            });
        }
        Ok(())
    }

    fn open_object(&mut self) -> io::Result<()> {
        let group = match self.stack.last() {
            None => Some(Group::Top),
            Some(Frame::Object(Object {
                group: Some(Group::Top),
                member: Some(member),
                ..
            })) if GROUPS.contains(&member.name.as_str()) => {
                Some(Group::Nested(member.name.clone()))
            }
            Some(Frame::Array(Array::Sources)) => {
                increment(&mut self.source_rows)?;
                None
            }
            Some(Frame::Array(Array::Uses)) => {
                increment(&mut self.use_rows)?;
                None
            }
            Some(Frame::Array(Array::References)) => {
                return Err(invalid("non-text source reference"));
            }
            _ => None,
        };
        self.stack.push(Frame::Object(Object {
            group,
            member: None,
            expect_key: true,
            reference_key: false,
        }));
        Ok(())
    }

    fn open_array(&mut self, at: u64) -> io::Result<bool> {
        let kind = match self.stack.last() {
            Some(Frame::Object(Object {
                reference_key: true,
                ..
            })) => {
                self.references.begin(at)?;
                Array::References
            }
            Some(Frame::Object(Object {
                group: Some(Group::Nested(group)),
                member: Some(member),
                ..
            })) if group == "provenance" => match member.name.as_str() {
                "sources" => {
                    start_inventory(&mut self.source_rows)?;
                    Array::Sources
                }
                "uses" => {
                    start_inventory(&mut self.use_rows)?;
                    Array::Uses
                }
                _ => Array::Other,
            },
            Some(Frame::Array(Array::References)) => {
                return Err(invalid("nested source reference array"));
            }
            _ => Array::Other,
        };
        self.stack.push(Frame::Array(kind));
        Ok(matches!(kind, Array::References))
    }

    fn close_object(&mut self, at: u64) -> io::Result<()> {
        self.end_member(at)?;
        let _closed = self.stack.pop();
        self.root_closed = self.stack.is_empty();
        Ok(())
    }

    fn end_member(&mut self, at: u64) -> io::Result<()> {
        let Some(Frame::Object(object)) = self.stack.last_mut() else {
            return Err(invalid("mismatched catalog object"));
        };
        let (Some(member), Some(group)) = (object.member.take(), &object.group) else {
            return Ok(());
        };
        let value_at = member
            .value_at
            .ok_or_else(|| invalid("missing section value start"))?;
        self.sections.push(Section {
            group: match group {
                Group::Top => None,
                Group::Nested(group) => Some(group.clone()),
            },
            name: member.name,
            value_bytes: at - value_at,
            member_bytes: at - member.key_at,
        });
        Ok(())
    }

    pub(super) fn finish(self) -> io::Result<Report> {
        if !self.root_closed
            || !self.stack.is_empty()
            || self.text.is_some()
            || self.references.active()
        {
            return Err(invalid("incomplete catalog stream"));
        }
        let top = self
            .sections
            .iter()
            .filter(|section| section.group.is_none());
        let top_count = top.clone().count() as u64;
        let top_bytes = top.map(|section| section.member_bytes).sum::<u64>();
        let framing = self
            .bytes
            .checked_sub(top_bytes)
            .ok_or_else(|| invalid("section lengths exceed decoded bytes"))?;
        if framing != top_count + 2 {
            return Err(invalid("compact catalog framing does not reconcile"));
        }
        Ok(Report {
            decoded_bytes: self.bytes,
            framing_bytes: framing,
            physical_sources: self
                .source_rows
                .ok_or_else(|| invalid("missing source inventory"))?,
            source_uses: self
                .use_rows
                .ok_or_else(|| invalid("missing source-use inventory"))?,
            sections: self.sections,
            reference_sets: self.references.report(),
        })
    }
}

impl Key {
    fn push(&mut self, byte: u8) -> io::Result<()> {
        if let Some(label) = &mut self.label {
            if !byte.is_ascii_alphanumeric() && byte != b'_' {
                return Err(invalid("non-schema section name"));
            }
            label.push(byte);
        }
        self.reference_bytes = self
            .reference_bytes
            .filter(|&offset| SOURCE_REFS.get(offset) == Some(&byte))
            .map(|offset| offset + 1);
        Ok(())
    }
}
