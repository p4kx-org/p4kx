// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::Archive;

use crate::input_budget::InputBudget;
use crate::shader_flags::{self, ShaderRead};
use crate::shader_index::ShaderIndex;
use crate::source_selection::SourceSelection;
use crate::{
    CandidateInventory, CandidateState, Digest, InputReceipt, MediaError, MediaWriter,
    OriginalIdentifier, Reference, ReferenceUnavailable, Subresource,
};

pub(crate) struct ShaderLookup<'run> {
    pub(crate) archive: &'run Archive,
    pub(crate) root: &'run Digest,
    pub(crate) selection: SourceSelection<'run>,
    pub(crate) budget: &'run InputBudget,
    pub(crate) limits: p4kx_shader::Limits,
    index: OnceCell<ShaderIndex>,
}

pub(crate) struct ShaderBinding<'lookup> {
    pub(crate) reference: Reference,
    pub(crate) input: Option<InputReceipt>,
    candidates: Vec<crate::shader_origins::ShaderCandidate>,
    pub(crate) origins: crate::shader_origins::ShaderReferenceOrigins,
    inventory: CandidateInventory,
    read: Option<&'lookup ShaderRead>,
}

impl<'run> ShaderLookup<'run> {
    pub(crate) fn new(
        archive: &'run Archive,
        root: &'run Digest,
        selection: SourceSelection<'run>,
        budget: &'run InputBudget,
    ) -> Self {
        Self {
            archive,
            root,
            selection,
            budget,
            limits: p4kx_shader::Limits::default(),
            index: OnceCell::new(),
        }
    }

    pub(crate) fn work(&self, count: usize, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        self.budget.metadata(
            0,
            crate::shader_values::count(count)?,
            writer.limits(),
            writer.checkpoint,
        )
    }

    fn index(&self, writer: &mut MediaWriter<'_>) -> Result<&ShaderIndex, MediaError> {
        if self.index.get().is_none() {
            let index = ShaderIndex::build(self, writer)?;
            self.index.set(index).map_err(|_| {
                MediaError::Contract("shader index initialized during its own construction".into())
            })?;
        }
        self.index.get().ok_or_else(|| {
            MediaError::Contract("shader index is unavailable after construction".into())
        })
    }

    pub(crate) fn shader(
        &self,
        name: &str,
        use_at: &Subresource,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ShaderBinding<'_>, MediaError> {
        let original = OriginalIdentifier::try_new_with_text("p4kx.shader.name", name)?;
        let Some(name) = shader_name(name, self.limits.token_bytes) else {
            return self.unavailable(original, ReferenceUnavailable::InvalidIdentifier, writer);
        };
        let index = self.index(writer)?;
        let matches = index.matching(name, self, writer)?;
        let mut candidates = index.candidates(&matches, writer)?;
        let read = self.unique_read(index, &matches, use_at, writer)?;
        if let (Some(value), Some(candidate)) = (read, candidates.first_mut()) {
            if value.supported {
                candidate.value.target.subresource = Some(Subresource::new(
                    crate::shader_projection::DEFINITION_NAMESPACE,
                    Vec::new(),
                ));
            } else {
                candidate.value.state = CandidateState::Unavailable {
                    reason: ReferenceUnavailable::UnsupportedLayout,
                };
            }
        }
        let (reference, evidence) = writer.try_resolve_reference_observed(
            original,
            candidates
                .iter()
                .map(|candidate| Ok((candidate.value.clone(), candidate.evidence.clone()))),
            index.inventory,
        )?;
        let semantic = if evidence.inventory_used
            || !evidence.retained.is_empty()
            || evidence.discarded.is_some()
        {
            crate::shader_read_origins::layout(read, &candidates, writer)?
        } else {
            crate::origin_contributions::OriginContributions::independent(&writer.origins.owner)
        };
        Ok(ShaderBinding {
            reference,
            input: receipt(read, use_at),
            candidates,
            inventory: index.inventory,
            origins: crate::shader_origins::ShaderReferenceOrigins {
                candidates: evidence,
                inventory: index.inventory_origins.clone(),
                semantic,
                read_selected: read.is_some(),
            },
            read,
        })
    }

    fn unique_read<'target>(
        &self,
        index: &'target ShaderIndex,
        matches: &[usize],
        use_at: &Subresource,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<&'target ShaderRead>, MediaError> {
        if matches.len() != 1 || !matches!(index.inventory, CandidateInventory::Complete) {
            return Ok(None);
        }
        let position = matches
            .first()
            .ok_or_else(|| MediaError::Contract("unique shader match is absent".into()))?;
        let target = index
            .targets
            .get(*position)
            .ok_or_else(|| MediaError::Contract("shader match is outside its index".into()))?;
        if !matches!(target.state, CandidateState::Selected) {
            return Ok(None);
        }
        if target.read.get().is_none() {
            let value = crate::shader_input::read(self, target, use_at, writer)?;
            target.read.set(value).map_err(|_| {
                MediaError::Contract("shader target initialized during its own read".into())
            })?;
        }
        Ok(target.read.get())
    }
    pub(crate) fn flag(
        &self,
        binding: &ShaderBinding<'_>,
        token: &str,
        use_at: &Subresource,
        writer: &mut MediaWriter<'_>,
    ) -> Result<crate::shader_origins::ShaderFlagBinding, MediaError> {
        let original = OriginalIdentifier::try_new_with_text("p4kx.shader.flag", token)?;
        let Some(name) =
            shader_flags::symbol(token).filter(|name| name.len() <= self.limits.token_bytes)
        else {
            let result =
                self.unavailable(original, ReferenceUnavailable::InvalidIdentifier, writer)?;
            return Ok(crate::shader_origins::ShaderFlagBinding {
                reference: result.reference,
                input: None,
                origins: result.origins,
            });
        };
        let Some(read) = binding.read.filter(|read| read.supported) else {
            let (reference, evidence) = writer.try_resolve_reference_observed(
                original,
                binding
                    .candidates
                    .iter()
                    .map(|candidate| Ok((candidate.value.clone(), candidate.evidence.clone()))),
                binding.inventory,
            )?;
            let semantic = if evidence.inventory_used
                || !evidence.retained.is_empty()
                || evidence.discarded.is_some()
            {
                crate::shader_read_origins::layout(binding.read, &binding.candidates, writer)?
            } else {
                crate::origin_contributions::OriginContributions::independent(&writer.origins.owner)
            };
            return Ok(crate::shader_origins::ShaderFlagBinding {
                reference,
                input: receipt(binding.read, use_at),
                origins: crate::shader_origins::ShaderReferenceOrigins {
                    candidates: evidence,
                    inventory: binding.origins.inventory.clone(),
                    semantic,
                    read_selected: binding.read.is_some(),
                },
            });
        };
        let (candidates, inventory) =
            crate::shader_lookup_flags::candidates(self, read, name, writer)?;
        let (reference, evidence) = writer.try_resolve_reference_observed(
            original,
            candidates
                .into_iter()
                .map(|candidate| Ok((candidate.value, candidate.evidence))),
            inventory,
        )?;
        let semantic = crate::shader_read_origins::flag(read, &evidence, writer)?;
        Ok(crate::shader_origins::ShaderFlagBinding {
            reference,
            input: receipt(Some(read), use_at),
            origins: crate::shader_origins::ShaderReferenceOrigins {
                candidates: evidence,
                inventory: binding.origins.inventory.clone(),
                semantic,
                read_selected: binding.read.is_some(),
            },
        })
    }

    fn unavailable(
        &self,
        original: OriginalIdentifier,
        reason: ReferenceUnavailable,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ShaderBinding<'_>, MediaError> {
        self.work(0, writer)?;
        let inventory = CandidateInventory::Unavailable(reason);
        let (reference, evidence) = writer
            .try_resolve_reference_observed::<crate::shader_origins::ShaderCandidateInput>(
                original,
                std::iter::empty(),
                inventory,
            )?;
        Ok(ShaderBinding {
            reference,
            origins: crate::shader_origins::ShaderReferenceOrigins {
                candidates: evidence,
                inventory: crate::origin_contributions::OriginContributions::independent(
                    &writer.origins.owner,
                ),
                semantic: crate::origin_contributions::OriginContributions::independent(
                    &writer.origins.owner,
                ),
                read_selected: false,
            },
            input: None,
            candidates: Vec::new(),
            inventory,
            read: None,
        })
    }
}

fn receipt(read: Option<&ShaderRead>, use_at: &Subresource) -> Option<InputReceipt> {
    read.and_then(|read| read.receipt.as_ref()).map(|receipt| {
        let mut receipt = receipt.clone();
        receipt.use_at = use_at.clone();
        receipt
    })
}

fn shader_name(value: &str, limit: usize) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && value.len() <= limit
        && !value.contains(['/', '\\', ':'])
        && !value.chars().any(char::is_control))
    .then_some(value)
}
