// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_gfx::{BitmapSource, FailureOrigin, TagSite};

use super::ui_extract_evidence::{Report, invalid};
use crate::ui_external::ExternalPath;
use crate::{ConversionError, SourceUse, SourceUseRole, UiSourceSite as Site};

use SourceUseRole::{Conversion, Payload, Selector};

impl Report<'_, '_> {
    pub(super) fn tag_failure(
        &mut self,
        index: usize,
        receipt: FailureOrigin,
    ) -> io::Result<Vec<SourceUse>> {
        let path = format!("/failed/{index}");
        let (classification, diagnostic) = match receipt {
            FailureOrigin::Tag(tag) => (
                self.tag_value(Site::Header(tag), "failedTagHeader", Conversion)?,
                self.tag_value(Site::Body(tag), "diagnostic", Conversion)?,
            ),
            FailureOrigin::Stream {
                header, available, ..
            } => {
                let mut actual =
                    self.tag_value(Site::Bytes(header), "retainedTagHeader", Conversion)?;
                actual.push(self.decoded_use(
                    Site::Survey(available),
                    "availableByteExtent",
                    Conversion,
                )?);
                (actual.clone(), actual)
            }
            _ => return Err(invalid("unrecognized original GFx diagnostic receipt")),
        };
        let complete = self.source.is_bound();
        for field in ["kind", "code", "offset"] {
            self.bind(&format!("{path}/{field}"), &classification, complete)?;
        }
        self.bind(&format!("{path}/error"), &diagnostic, complete)?;
        let all = classification
            .into_iter()
            .chain(diagnostic)
            .collect::<Vec<_>>();
        self.bind(&path, &all, complete)?;
        Ok(all)
    }

    pub(super) fn failure(
        &mut self,
        index: usize,
        order: usize,
        tag: TagSite,
        error: &ConversionError,
    ) -> io::Result<(Vec<SourceUse>, bool)> {
        let path = format!("/failed/{index}");
        let kind = self.tag_value(Site::Header(tag), "code", Conversion)?;
        let id = self.tag_value(Site::Body(tag), "id", Payload)?;
        self.bind(&format!("{path}/kind"), &kind, self.source.is_bound())?;
        self.bind(&format!("{path}/id"), &id, self.source.is_bound())?;
        let mut uses = kind.into_iter().chain(id).collect::<Vec<_>>();
        let complete = match error {
            ConversionError::ExternalImageMissing { .. }
            | ConversionError::ExternalImageOutside { .. } => {
                let bitmap =
                    self.decoded.assets.bitmaps.get(order).ok_or_else(|| {
                        invalid("external image failure has no bitmap definition")
                    })?;
                let BitmapSource::External(image) = &bitmap.source else {
                    return Err(invalid(
                        "external image failure names another kind of bitmap",
                    ));
                };
                self.external_selectors(&mut uses, tag, ExternalPath::select(image))?;
                uses.push(self.source.name_use().map_err(io::Error::other)?);
                self.source.is_bound()
            }
            ConversionError::UiSubImage { .. } => {
                uses.push(self.decoded_use(Site::Body(tag), "atlasImageId", Selector)?);
                uses.push(self.source.name_use().map_err(io::Error::other)?);
                false
            }
            ConversionError::Write { .. } => self.source.is_bound(),
            _ => false,
        };
        self.bind(&format!("{path}/error"), &uses, complete)?;
        self.bind(&path, &uses, complete)?;
        Ok((uses, complete))
    }
}
