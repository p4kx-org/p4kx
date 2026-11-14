// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, SourceDeclarationEvidence, SourceEncryptionState, SourceRecordFormat, SourceValue,
};

use crate::MediaError;
use crate::origin_fact_read::{Declaration, Fact};
use crate::origin_inventory_wire::{DirectoryOrdinal, Encryption, Scope, SourceKind, SourceRow};
use crate::origin_read_policy::{InventoryBudget, OriginReadPolicy};
use crate::origin_source_shape::RecordFormat;

/// Retained inventory descriptions without an original source binding.
pub(crate) struct SourceFacts {
    pub(crate) ancestors: Vec<String>,
    pub(crate) stored_encryption: Encryption,
    pub(crate) kind: SourceKind,
    pub(crate) admitted_entry_count: Option<DirectoryOrdinal>,
}

impl SourceFacts {
    /// Checks captured domain metadata against the retained physical description.
    pub(crate) fn accepts_encryption(
        &self,
        scope: Scope,
        ancestors: Encryption,
        captured: Encryption,
    ) -> bool {
        let expected = match (self.kind, scope) {
            (SourceKind::File | SourceKind::ArchiveFile, _) => Encryption::Unknown,
            (SourceKind::ArchiveDirectory, Scope::ArchiveDirectoryMetadata) => {
                Encryption::Unencrypted
            }
            (SourceKind::ArchiveDirectory, _) => return false,
            (SourceKind::Unknown, Scope::ArchiveDirectoryMetadata | Scope::FileMetadata)
                if matches!(ancestors, Encryption::Unencrypted) =>
            {
                return matches!(captured, Encryption::Unencrypted | Encryption::Unknown);
            }
            (_, Scope::Payload) => ancestors.combine(self.stored_encryption),
            (_, Scope::ArchiveDirectoryMetadata | Scope::FileMetadata) => ancestors,
        };
        captured == expected
    }

    pub(crate) fn capture(
        mut source: SourceRow,
        policy: OriginReadPolicy,
        budget: &mut InventoryBudget,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<(String, Self), MediaError> {
        source.require_shape()?;
        capture(source.path.take(), policy, budget, origins)?;
        capture(source.name.take(), policy, budget, origins)?;
        capture(source.byte_length.take(), policy, budget, origins)?;
        let zip_flags = match source.zip_flags.take() {
            Some(fact) => fact.capture_value(policy.fields, budget, origins)?,
            None => SourceValue::Unreadable,
        };
        let declarations = declarations(source.declarations.take(), policy, budget, origins)?;
        require_declaration_state(
            source.stored_encryption,
            source.record_format.as_ref().map(RecordFormat::shared),
            zip_flags,
            declarations,
        )?;
        Ok((
            source.id,
            Self {
                ancestors: source.ancestors,
                stored_encryption: source.stored_encryption,
                kind: source.kind,
                admitted_entry_count: source.admitted_entry_count,
            },
        ))
    }
}

fn capture<T>(
    fact: Option<Fact<T>>,
    policy: OriginReadPolicy,
    budget: &mut InventoryBudget,
    origins: &mut Vec<FieldOrigin>,
) -> Result<(), MediaError> {
    if let Some(fact) = fact {
        fact.capture_origin(policy.fields, budget, origins)?;
    }
    Ok(())
}

fn declarations(
    fact: Option<Fact<Vec<Fact<Declaration>>>>,
    policy: OriginReadPolicy,
    budget: &mut InventoryBudget,
    origins: &mut Vec<FieldOrigin>,
) -> Result<SourceValue<Vec<SourceDeclarationEvidence>>, MediaError> {
    let Some(fact) = fact else {
        return Ok(SourceValue::Unreadable);
    };
    let declarations = match fact.capture_value(policy.fields, budget, origins)? {
        SourceValue::Present(declarations) => declarations,
        SourceValue::Absent => return Ok(SourceValue::Absent),
        SourceValue::Null => return Ok(SourceValue::Null),
        SourceValue::Invalid => return Ok(SourceValue::Invalid),
        SourceValue::Unsupported => return Ok(SourceValue::Unsupported),
        SourceValue::Unreadable | _ => return Ok(SourceValue::Unreadable),
    };
    let mut evidence = Vec::new();
    for fact in declarations {
        let declaration = match fact.capture_value(policy.fields, budget, origins)? {
            SourceValue::Present(declaration) => {
                declaration.capture(policy.fields, budget, origins)?
            }
            SourceValue::Absent => continue,
            SourceValue::Null | SourceValue::Invalid | SourceValue::Unsupported => {
                SourceDeclarationEvidence::unsupported()
            }
            SourceValue::Unreadable | _ => SourceDeclarationEvidence::unavailable(),
        };
        evidence.push(declaration);
    }
    Ok(SourceValue::Present(evidence))
}

fn require_declaration_state(
    stored: Encryption,
    format: Option<SourceRecordFormat>,
    zip_flags: SourceValue<u16>,
    declarations: SourceValue<Vec<SourceDeclarationEvidence>>,
) -> Result<(), MediaError> {
    let stored = match stored {
        Encryption::Encrypted => SourceEncryptionState::Encrypted,
        Encryption::Unencrypted => SourceEncryptionState::Unencrypted,
        Encryption::Unknown => SourceEncryptionState::Unknown,
    };
    let (declarations, fallback) = match declarations {
        SourceValue::Present(declarations) => (declarations, None),
        SourceValue::Absent => (Vec::new(), None),
        SourceValue::Null | SourceValue::Invalid | SourceValue::Unsupported => {
            (Vec::new(), Some(SourceDeclarationEvidence::unsupported()))
        }
        SourceValue::Unreadable | _ => (Vec::new(), Some(SourceDeclarationEvidence::unavailable())),
    };
    let declarations = declarations.into_iter().chain(fallback);
    if !stored.is_possible_from_evidence(format, zip_flags, declarations) {
        return Err(MediaError::Destination(
            "stored source encryption contradicts available directory declarations".into(),
        ));
    }
    Ok(())
}
