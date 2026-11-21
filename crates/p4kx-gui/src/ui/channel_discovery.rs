// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use crate::channel_refresh::RefreshFailure;
use crate::{Model, Text};

pub(super) fn show(ui: &mut Ui, model: &Model) {
    let issue = model
        .channels()
        .discovery_issues()
        .first()
        .or_else(|| model.comparison().discovery_issues().first());
    let detail = issue.map(ToString::to_string).or_else(|| {
        model
            .channel_refresh_failure()
            .map(|failure| match failure {
                RefreshFailure::Spawn(error) => error.to_string(),
                RefreshFailure::Stopped => model.texts().get(Text::ChannelsDiscoveryStopped),
            })
    });
    let Some(detail) = detail else {
        return;
    };
    let notice = model.texts().get(Text::ChannelsDiscoveryIncomplete);
    super::warning_notice::show(ui, "channel-discovery", &notice, Some(&detail));
}
