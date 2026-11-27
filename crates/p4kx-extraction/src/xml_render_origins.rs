// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_cryxml::{ByteOrder, XmlChildAbsence, XmlRenderKind, XmlRenderSpan};

use crate::{
    FieldOrigin, InputScope, ObservedEntry, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

/// Source evidence for an emitted portion of rendered XML.
#[derive(Debug)]
pub struct XmlRenderOrigin {
    output: Range<u64>,
    kind: XmlRenderKind,
    origin: FieldOrigin,
}

impl XmlRenderOrigin {
    /// Returns the half-open range in the retained rendered bytes.
    #[must_use]
    pub const fn output(&self) -> &Range<u64> {
        &self.output
    }

    /// Returns the kind of the rendered portion.
    #[must_use]
    pub const fn kind(&self) -> XmlRenderKind {
        self.kind
    }

    /// Returns the retained original contributors and immediate encryption marker.
    #[must_use]
    pub const fn origin(&self) -> &FieldOrigin {
        &self.origin
    }
}

/// Exact original evidence for the source-backed portions of rendered CryXml.
#[derive(Debug)]
pub struct XmlRenderOrigins {
    spans: Vec<XmlRenderOrigin>,
    inventory: SourceInventory,
    contains_encrypted_source: Option<bool>,
}

impl XmlRenderOrigins {
    /// Captures the positioned origins for the complete lines retained by the caller.
    /// The positions must come from the same render of the observed Archive Entry.
    ///
    /// # Errors
    /// Returns an error if the original source evidence cannot be registered.
    pub fn retained(
        original: Option<ObservedEntry<'_>>,
        spans: impl IntoIterator<Item = XmlRenderSpan>,
        retained_end: u64,
    ) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::with_compact_use_refs();
        let complete = original.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let source = original.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let scope = uses.observe(source, InputScope::Payload)?;
        let spans = spans
            .into_iter()
            .filter(|span| span.range().start < retained_end && span.range().end <= retained_end)
            .map(|span| capture_span(&uses, &scope, &span, complete))
            .collect::<Result<Vec<_>, _>>()?;
        let contains_encrypted_source = display_marker(&spans);
        Ok(Self {
            spans,
            inventory: uses.freeze()?,
            contains_encrypted_source,
        })
    }

    /// Returns the positioned evidence in render order.
    #[must_use]
    pub fn spans(&self) -> &[XmlRenderOrigin] {
        &self.spans
    }

    /// Returns the encryption marker of the retained source-dependent spans.
    #[must_use]
    pub const fn contains_encrypted_source(&self) -> Option<bool> {
        self.contains_encrypted_source
    }

    /// Returns the physical source inventory of the retained references.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

fn display_marker(spans: &[XmlRenderOrigin]) -> Option<bool> {
    let mut any = false;
    let mut complete = true;
    for span in spans {
        let origin = &span.origin;
        if origin.source_uses_refs().is_empty() && origin.source_set_complete() {
            continue;
        }
        any = true;
        let marker = origin.contains_encrypted_source();
        if marker == Some(true) {
            return marker;
        }
        complete &= marker == Some(false);
    }
    (any && complete).then_some(false)
}

fn capture_span(
    uses: &SourceUses,
    scope: &SourceScope,
    span: &XmlRenderSpan,
    bound: bool,
) -> Result<XmlRenderOrigin, SourceEvidenceError> {
    let role_origin = if let Some(selection) = span.attribute_selection() {
        let proof = scope.cryxml_field(selection)?;
        let complete = bound && proof.is_some();
        uses.origin(&proof.map_or_else(Vec::new, |proof| proof.uses), complete)
    } else {
        contiguous_origin(uses, scope, span, bound)?
    };
    Ok(XmlRenderOrigin {
        output: span.range().clone(),
        kind: span.kind(),
        origin: role_origin,
    })
}

fn contiguous_origin(
    uses: &SourceUses,
    scope: &SourceScope,
    span: &XmlRenderSpan,
    bound: bool,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let mut contributors: Vec<SourceUse> = span
        .sources()
        .map(|fact| {
            let role = if fact.kind().is_selector() {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Conversion
            };
            scope.at(
                SourceLocation::archive_entry_decoded_bytes(
                    fact.range().clone(),
                    fact.kind().as_str(),
                ),
                role,
            )
        })
        .collect::<Result<_, _>>()?;
    let absence = span.child_absence();
    if let Some(proof) = absence {
        contributors.push(scope.at(child_absence(proof), SourceUseRole::Selector)?);
    }
    let generated = contributors.is_empty() && span.complete();
    let complete = generated || (bound && (span.complete() || absence.is_some()));
    Ok(uses.origin(&contributors, complete))
}

fn child_absence(proof: &XmlChildAbsence) -> SourceLocation {
    let directory = proof.node_directory();
    let mut indices = vec![
        u64::from(proof.node_index()),
        directory.start,
        directory.end,
    ];
    if let Some(words) = proof.parent_words() {
        indices.extend([
            words.first_offset(),
            words.word_count().into(),
            words.stride(),
            words.word_width(),
        ]);
    }
    let field = match proof.byte_order() {
        ByteOrder::Little => "littleEndian",
        ByteOrder::Big => "bigEndian",
    };
    SourceLocation::Subresource {
        namespace: "cryXmlChildAbsence".into(),
        indices,
        field: field.into(),
    }
}
