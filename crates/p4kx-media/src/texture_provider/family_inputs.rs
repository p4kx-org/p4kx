// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;

use super::family_payload::Relation;
use crate::origin_contributions::OriginContributions;
use crate::{CandidateInventory, DirectoryLookup, EntryInput, ReferenceUnavailable};

#[derive(Debug)]
pub(super) struct FamilyInputs {
    members: ObservedMembers,
    refusal: Option<ReferenceUnavailable>,
    evidence: OriginContributions,
}

#[derive(Debug, Default)]
pub(super) struct ObservedMembers {
    alpha_header: ObservedMember,
    color_parts: [ObservedMember; u32::BITS as usize],
    alpha_parts: [ObservedMember; u32::BITS as usize],
}

#[derive(Debug, Clone, Copy, Default)]
enum ObservedMember {
    #[default]
    Missing,
    Unique {
        index: usize,
        size: u64,
        encrypted: bool,
    },
    Ambiguous,
}
#[derive(Debug)]
pub(super) struct FamilyInput<'a> {
    pub(super) token: EntryInput<'a>,
    pub(super) size: u64,
}

#[derive(Debug)]
pub(super) enum InputRefusal {
    Missing,
    Ambiguous,
    Unproved(ReferenceUnavailable),
    KeyRequired,
    WrongKey,
    Unreadable,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MipBranch {
    Color,
    Alpha,
}

impl ObservedMembers {
    pub(super) fn observe(&mut self, relation: Relation, entry: &Entry) {
        let member = match relation {
            Relation::AlphaHeader => Some(&mut self.alpha_header),
            Relation::ColorPart { number } => {
                part_index(number).and_then(|index| self.color_parts.get_mut(index))
            }
            Relation::AlphaPart { number } => {
                part_index(number).and_then(|index| self.alpha_parts.get_mut(index))
            }
            Relation::ColorHeader | Relation::UnprovedPath => None,
        };
        if let Some(member) = member {
            *member = match member {
                ObservedMember::Missing => ObservedMember::Unique {
                    index: entry.index(),
                    size: entry.uncompressed_size(),
                    encrypted: entry.is_encrypted(),
                },
                ObservedMember::Unique { .. } | ObservedMember::Ambiguous => {
                    ObservedMember::Ambiguous
                }
            };
        }
    }

    pub(super) const fn finish(
        self,
        inventory: CandidateInventory,
        lossless: bool,
        evidence: OriginContributions,
    ) -> FamilyInputs {
        let refusal = match inventory {
            CandidateInventory::Unavailable(reason) => Some(reason),
            CandidateInventory::Complete if !lossless => {
                Some(ReferenceUnavailable::UnsupportedLayout)
            }
            CandidateInventory::Complete => None,
        };
        FamilyInputs {
            members: self,
            refusal,
            evidence,
        }
    }
}

impl FamilyInputs {
    pub(super) const fn evidence(&self) -> &OriginContributions {
        &self.evidence
    }
    pub(super) fn alpha_header<'a>(
        &self,
        directory: &'a DirectoryLookup<'_>,
    ) -> Result<FamilyInput<'a>, InputRefusal> {
        self.prove(Some(&self.members.alpha_header), directory)
    }

    pub(super) fn part<'a>(
        &self,
        branch: MipBranch,
        number: u32,
        directory: &'a DirectoryLookup<'_>,
    ) -> Result<FamilyInput<'a>, InputRefusal> {
        let parts = match branch {
            MipBranch::Color => &self.members.color_parts,
            MipBranch::Alpha => &self.members.alpha_parts,
        };
        self.prove(
            part_index(number).and_then(|index| parts.get(index)),
            directory,
        )
    }

    fn prove<'a>(
        &self,
        member: Option<&ObservedMember>,
        directory: &'a DirectoryLookup<'_>,
    ) -> Result<FamilyInput<'a>, InputRefusal> {
        match member {
            Some(ObservedMember::Ambiguous) => return Err(InputRefusal::Ambiguous),
            _ if let Some(reason) = self.refusal => return Err(InputRefusal::Unproved(reason)),
            _ => {}
        }
        let Some(ObservedMember::Unique {
            index,
            size,
            encrypted,
        }) = member
        else {
            return Err(InputRefusal::Missing);
        };
        if *encrypted && !directory.index.archive.has_key() {
            return Err(InputRefusal::KeyRequired);
        }
        Ok(FamilyInput {
            token: EntryInput {
                root: directory.root,
                chain: directory.chain,
                position: *index,
            },
            size: *size,
        })
    }
}

const fn part_index(number: u32) -> Option<usize> {
    match number.checked_sub(1) {
        Some(index) => Some(index as usize),
        None => None,
    }
}
