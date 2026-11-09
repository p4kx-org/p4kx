// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    EntryOrigin, EntryOriginStep, ObservedEntry, ObservedEntryBinding, SourceEncryption,
};

use crate::DecimalU64;
use crate::source_archive_binding::{ArchiveSourceBinding, SourceNonce};
use crate::source_details::SourceDetails;
use crate::source_inventory::{
    ArchiveEntryProof, PhysicalSource, PhysicalSourceKind, PhysicalSourceRef, SourceEncryptionState,
};
use crate::source_registration;
use crate::source_registry::{InputIdentity, RegisteredSource, SourceRegistry};

#[derive(Clone, Copy)]
enum IdentityPolicy {
    Legacy,
    StableView,
}

#[derive(Clone, Copy)]
enum ArchiveRegistration {
    Fresh(SourceNonce),
    Reusable(SourceNonce),
}

impl ArchiveRegistration {
    const fn nonce(self) -> SourceNonce {
        match self {
            Self::Fresh(nonce) | Self::Reusable(nonce) => nonce,
        }
    }

    const fn may_reuse(self) -> bool {
        matches!(self, Self::Reusable(_))
    }
}

enum Step<'evidence> {
    Owned(&'evidence EntryOriginStep),
    Borrowed(ObservedEntry<'evidence>),
}

impl Step<'_> {
    const fn index(&self) -> usize {
        match self {
            Self::Owned(step) => step.index(),
            Self::Borrowed(view) => view.entry().index(),
        }
    }

    fn ordinal(&self) -> u64 {
        match self {
            Self::Owned(step) => u64::from(step.ordinal()),
            Self::Borrowed(view) => u64::from(view.entry_index()),
        }
    }

    fn encryption(&self) -> SourceEncryption {
        match self {
            Self::Owned(step) => step.encryption(),
            Self::Borrowed(view) => view.stored_encryption(),
        }
    }
}

impl SourceRegistry {
    pub(crate) fn archive(
        &mut self,
        origin: &EntryOrigin,
        details: SourceDetails,
    ) -> RegisteredSource {
        let input = self.archive_input(origin.input_origin());
        let binding = origin.binding();
        let registration =
            self.archive_registration(input, &binding, details, IdentityPolicy::Legacy);
        self.archive_steps(
            input,
            registration,
            &binding,
            origin
                .ancestors()
                .map(|step| step.map(Step::Owned))
                .chain(std::iter::once(Some(Step::Owned(origin.entry())))),
            details,
            IdentityPolicy::Legacy,
        )
    }

    pub(crate) fn archive_view(
        &mut self,
        view: ObservedEntry<'_>,
        details: SourceDetails,
    ) -> RegisteredSource {
        let binding = view.binding();
        let input = self.archive_input(view.input_origin());
        let registration =
            self.archive_registration(input, &binding, details, IdentityPolicy::StableView);
        self.archive_steps(
            input,
            registration,
            &binding,
            view.ancestors()
                .map(|step| step.map(Step::Owned))
                .chain(std::iter::once(Some(Step::Borrowed(view)))),
            details,
            IdentityPolicy::StableView,
        )
    }

    fn archive_registration(
        &mut self,
        input: usize,
        binding: &ObservedEntryBinding,
        details: SourceDetails,
        policy: IdentityPolicy,
    ) -> ArchiveRegistration {
        if !binding.has_unknown_ancestry() {
            return ArchiveRegistration::Reusable(SourceNonce::Known);
        }
        if let Some(known) = self.inputs.iter().find_map(|known| match known {
            InputIdentity::ArchiveView(known) if known.original.is_same_view(binding) => {
                Some(known.nonce)
            }
            _ => None,
        }) {
            return ArchiveRegistration::Reusable(known);
        }
        if matches!(policy, IdentityPolicy::Legacy) && details == SourceDetails::Full {
            return ArchiveRegistration::Fresh(SourceNonce::Legacy(self.sources.len()));
        }
        let nonce = self
            .indexed_archive_source(input, binding)
            .and_then(|source| source.binding.as_ref())
            .map(|known| known.nonce)
            .or_else(|| {
                self.sources
                    .values()
                    .filter_map(|source| source.binding.as_ref())
                    .find(|known| known.original.is_same_view(binding))
                    .map(|known| known.nonce)
            })
            .unwrap_or(SourceNonce::View(self.inputs.len()));
        self.inputs
            .push(InputIdentity::ArchiveView(ArchiveSourceBinding {
                original: binding.clone(),
                nonce,
            }));
        ArchiveRegistration::Reusable(nonce)
    }

    fn archive_steps<'evidence>(
        &mut self,
        input: usize,
        registration: ArchiveRegistration,
        binding: &ObservedEntryBinding,
        steps: impl Iterator<Item = Option<Step<'evidence>>>,
        details: SourceDetails,
        policy: IdentityPolicy,
    ) -> RegisteredSource {
        use std::fmt::Write as _;

        let nonce = registration.nonce();
        let mut ancestors = Vec::new();
        let mut pending = Vec::new();
        let mut path = String::new();
        let mut effective = SourceEncryptionState::Unencrypted;
        let mut metadata = effective;
        let mut id = PhysicalSourceRef(String::new());
        let leaf = binding.ancestor_count();
        for (position, step) in steps.enumerate() {
            metadata = effective;
            if let Some(step) = step {
                let _formatted = write!(path, ".{}", step.index());
                let original = if position == leaf {
                    Some(binding.clone())
                } else {
                    binding.ancestor(position)
                };
                let existing = original
                    .as_ref()
                    .filter(|_| position != leaf || registration.may_reuse())
                    .and_then(|original| self.archive_prefix(input, original));
                let was_indexed = existing.is_some();
                let step_nonce;
                if let Some((reference, prefix, chosen)) = existing {
                    id = reference;
                    ancestors = prefix;
                    pending.clear();
                    step_nonce = chosen;
                } else {
                    step_nonce = if position == leaf {
                        nonce
                    } else {
                        self.ancestor_nonce(input, original.as_ref(), details, policy, nonce)
                    };
                    let key = step_nonce.key();
                    self.unknown_prefixes(input, &key, &mut pending, &mut ancestors);
                    id = PhysicalSourceRef(format!("a{input}{key}{path}"));
                }
                let original = original.map(|original| ArchiveSourceBinding {
                    original,
                    nonce: step_nonce,
                });
                self.archive_step(&id, &ancestors, &step, metadata, details, original);
                if !was_indexed {
                    self.index_archive_source(input, &id);
                }
                effective = effective.combine(SourceEncryptionState::of(step.encryption()));
                ancestors.push(id.clone());
            } else {
                let _formatted = write!(path, ".?{position}");
                pending.push(path.clone());
                effective = effective.combine(SourceEncryptionState::Unknown);
            }
        }
        RegisteredSource {
            id,
            payload: effective,
            metadata,
        }
    }

    fn unknown_prefixes(
        &mut self,
        input: usize,
        key: &str,
        pending: &mut Vec<String>,
        ancestors: &mut Vec<PhysicalSourceRef>,
    ) {
        for prefix in pending.iter() {
            let unknown = PhysicalSourceRef(format!("a{input}{key}{prefix}"));
            self.unknown(unknown.clone(), ancestors.clone());
            ancestors.push(unknown);
        }
        pending.clear();
    }

    fn ancestor_nonce(
        &mut self,
        input: usize,
        original: Option<&ObservedEntryBinding>,
        details: SourceDetails,
        policy: IdentityPolicy,
        nonce: SourceNonce,
    ) -> SourceNonce {
        if matches!(policy, IdentityPolicy::Legacy) && matches!(nonce, SourceNonce::Legacy(_)) {
            nonce
        } else {
            original.map_or(nonce, |original| {
                self.archive_registration(input, original, details, policy)
                    .nonce()
            })
        }
    }

    fn archive_prefix(
        &self,
        input: usize,
        original: &ObservedEntryBinding,
    ) -> Option<(PhysicalSourceRef, Vec<PhysicalSourceRef>, SourceNonce)> {
        let source = self.indexed_archive_source(input, original)?;
        let known = source.binding.as_ref()?;
        Some((source.id.clone(), source.ancestors.clone(), known.nonce))
    }

    fn indexed_archive_source(
        &self,
        input: usize,
        original: &ObservedEntryBinding,
    ) -> Option<&PhysicalSource> {
        let InputIdentity::Archive { source_refs, .. } = self.inputs.get(input)? else {
            return None;
        };
        let source = self.sources.get(source_refs.get(original)?)?;
        source
            .binding
            .as_ref()
            .filter(|known| known.matches(original))
            .map(|_known| source)
    }

    fn index_archive_source(&mut self, input: usize, reference: &PhysicalSourceRef) {
        let Some(original) = self
            .sources
            .get(reference)
            .and_then(|source| source.binding.as_ref())
            .map(|known| &known.original)
        else {
            return;
        };
        if let Some(InputIdentity::Archive { source_refs, .. }) = self.inputs.get_mut(input) {
            source_refs
                .entry(original.clone())
                .or_insert_with(|| reference.clone());
        }
    }

    fn archive_step(
        &mut self,
        id: &PhysicalSourceRef,
        ancestors: &[PhysicalSourceRef],
        step: &Step<'_>,
        metadata: SourceEncryptionState,
        details: SourceDetails,
        binding: Option<ArchiveSourceBinding>,
    ) {
        let upgrade = self.sources.get(id).is_some_and(|source| {
            matches!(source.source, PhysicalSourceKind::ArchiveEntryProof(_))
        }) && details == SourceDetails::Full;
        if !self.sources.contains_key(id) || upgrade {
            let mut source = match details {
                SourceDetails::Full => match step {
                    Step::Owned(step) => source_registration::archive(
                        self,
                        id.clone(),
                        ancestors.to_vec(),
                        step,
                        metadata,
                    ),
                    Step::Borrowed(view) => {
                        let full = view.snapshot();
                        source_registration::archive(
                            self,
                            id.clone(),
                            ancestors.to_vec(),
                            full.entry(),
                            metadata,
                        )
                    }
                },
                SourceDetails::CaptureLimited => PhysicalSource {
                    id: id.clone(),
                    ancestors: ancestors.to_vec(),
                    stored_encryption: SourceEncryptionState::of(step.encryption()),
                    binding: None,
                    source: PhysicalSourceKind::ArchiveEntryProof(ArchiveEntryProof {
                        entry_index: DecimalU64::new(step.ordinal()),
                        details,
                    }),
                },
            };
            source.binding = binding;
            self.sources.insert(id.clone(), source);
        } else if let Some(source) = self.sources.get_mut(id)
            && source.binding.is_none()
        {
            source.binding = binding;
        }
    }
}
