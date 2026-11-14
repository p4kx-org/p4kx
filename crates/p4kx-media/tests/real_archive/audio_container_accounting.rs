// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::hirc_view::{Detail, ObjectView};
use p4kx_extraction::{HircValue, SectionId, Soundbank};
use p4kx_media::{MediaRecord, Status};
use serde_json::{Value, json};

use super::{TestResult, at, range, require};

const CONTAINER: &str = "audio-bank-hirc-container";
const CHILD: &str = "audio-bank-hirc-container-child";
const WORD_BYTES: usize = size_of::<u32>();

#[derive(Default)]
pub(super) struct Counts {
    pub(super) containers: usize,
    pub(super) children: usize,
}

pub(super) fn has_containers_with_children(input: &[u8]) -> bool {
    Soundbank::parse(input).is_ok_and(|bank| {
        bank.objects().iter().any(|object| {
            matches!(&object.value, HircValue::Container(container) if !container.children.is_empty())
        }) && bank.objects().iter().all(|object| !matches!(&object.value, HircValue::Malformed(_)))
    })
}

pub(super) fn verify(
    bank: &Soundbank<&[u8]>,
    original: &[u8],
    rows: &[MediaRecord],
) -> TestResult<Counts> {
    let section = bank
        .sections()
        .position(|section| section.id == SectionId::HIRC)
        .ok_or("source HIRC section")?;
    let section = u64::try_from(section)?;
    let mut containers = at(rows, CONTAINER);
    let mut children = at(rows, CHILD);
    let mut counts = Counts::default();
    for (position, object) in bank.objects().iter().enumerate() {
        if !matches!(&object.value, HircValue::Container(_)) {
            continue;
        }
        let view = ObjectView::new(position, object);
        let position = u64::try_from(view.index)?;
        let row = containers.next().ok_or("expected container row")?;
        require(
            row.subresource.indices == [section, position],
            "original container position",
        )?;
        let words = verify_container(&view, row, original, bank.object_body(object))?;
        for (ordinal, (id, word)) in view.children.into_iter().zip(words).enumerate() {
            let ordinal = u64::try_from(ordinal)?;
            let row = children.next().ok_or("expected container child row")?;
            verify_child(row, [section, position, ordinal], id, word, original)?;
            counts.children += 1;
        }
        counts.containers += 1;
    }
    require(containers.next().is_none(), "no surplus container rows")?;
    require(children.next().is_none(), "no surplus container child rows")?;
    require(
        counts.containers > 0 && counts.children > 0,
        "nonempty container evidence",
    )?;
    Ok(counts)
}

fn verify_container<'input>(
    view: &ObjectView<'_>,
    row: &MediaRecord,
    original: &'input [u8],
    body: &[u8],
) -> TestResult<&'input [[u8; WORD_BYTES]]> {
    let Some(Detail::Container(detail)) = &view.detail else {
        return Err("original container detail is unavailable".into());
    };
    require(
        row.status == Status::Complete,
        "decoded container is complete",
    )?;
    require(
        row.data.pointer("/payload/layout") == Some(&json!("decoded"))
            && row.data.pointer("/payload/container_kind") == Some(&json!(detail.container_kind))
            && row.data.pointer("/payload/parent_id") == Some(&json!(view.parent.unwrap_or(0)))
            && row.data.pointer("/payload/child_count") == Some(&json!(view.children.len())),
        "original layout, kind, parent and child count",
    )?;
    if matches!(detail.container_kind, "switch" | "blend") {
        require(
            row.data.pointer("/payload/mode_code") == Some(&Value::Null),
            "layout without a random or sequence mode",
        )?;
    }
    let span = range(row, "/payload/children_range", original)?;
    let child_span = span.as_ptr_range();
    let object_span = body.as_ptr_range();
    require(
        child_span.start >= object_span.start && child_span.end <= object_span.end,
        "children are within the original HIRC body",
    )?;
    let (words, suffix) = span.as_chunks::<WORD_BYTES>();
    require(
        suffix.is_empty() && words.len() == view.children.len(),
        "original child word count",
    )?;
    Ok(words)
}

fn verify_child(
    row: &MediaRecord,
    position: [u64; 3],
    id: u32,
    word: &[u8; WORD_BYTES],
    original: &[u8],
) -> TestResult {
    let [_, _, ordinal] = position;
    require(
        row.status == Status::Complete
            && row.subresource.indices == position
            && row.data.pointer("/payload/ordinal") == Some(&json!(ordinal.to_string())),
        "original child position",
    )?;
    require(
        u32::from_le_bytes(*word) == id
            && row.data.pointer("/payload/child_id") == Some(&json!(id)),
        "original ordered child ID",
    )?;
    require(
        std::ptr::eq(range(row, "/payload/id_range", original)?, word.as_slice()),
        "child range names its exact original word",
    )
}
