// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::engine::general_purpose::STANDARD;
use p4kx_extraction::CancelToken;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes;

pub(crate) fn encode(bytes: &[u8], cancel: &CancelToken) -> Result<String, SocSceneError> {
    use base64::Engine as _;

    const BLOCK_LEN: usize = 3 * 16_384;
    let mut result = String::new();
    let encoded_len = base64::encoded_len(bytes.len(), true).ok_or(SocSceneError::Position)?;
    result.try_reserve_exact(encoded_len)?;
    for block in bytes.chunks(BLOCK_LEN) {
        soc_scene_classes::check(cancel)?;
        STANDARD.encode_string(block, &mut result);
    }
    Ok(result)
}
