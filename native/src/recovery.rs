//! A single atomic recovery envelope couples the document to its session metadata.
use crate::{
    format::{self, DecodedPlan},
    model::{PlanFile, ProjectAsset},
    storage,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const HEADER: &[u8] = b"RPP-RECOVERY-1\n";
const MAX_METADATA: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
struct Metadata {
    source: Option<PathBuf>,
    unsaved: bool,
}

pub fn save(
    path: &Path,
    plan: &PlanFile,
    assets: &[ProjectAsset],
    source: Option<&Path>,
    unsaved: bool,
) -> Result<(), String> {
    let metadata = serde_json::to_vec(&Metadata {
        source: source.map(Path::to_path_buf),
        unsaved,
    })
    .map_err(|error| error.to_string())?;
    if metadata.len() > MAX_METADATA {
        return Err("Recovery metadata exceeds the size limit".into());
    }
    let mut bytes = HEADER.to_vec();
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&metadata);
    if unsaved {
        bytes.extend_from_slice(
            &format::encode_project(plan, assets).map_err(|error| error.to_string())?,
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    storage::write_atomic(path, &bytes).map_err(|error| error.to_string())
}

pub fn load(path: &Path) -> Result<Option<(DecodedPlan, Option<PathBuf>)>, String> {
    let bytes = storage::read_limited(
        path,
        format::MAX_PROJECT_BYTES + MAX_METADATA + HEADER.len() + 4,
    )
    .map_err(|error| error.to_string())?;
    let Some(body) = bytes.strip_prefix(HEADER) else {
        // Older recoveries have no session information. Preserve their recoverability.
        return format::decode_plan(&bytes)
            .map(|plan| Some((plan, None)))
            .map_err(|error| error.to_string());
    };
    let length = body.get(..4).ok_or("Truncated recovery metadata")?;
    let length =
        u32::from_le_bytes(length.try_into().map_err(|_| "Invalid recovery length")?) as usize;
    if length > MAX_METADATA {
        return Err("Recovery metadata exceeds the size limit".into());
    }
    let metadata: Metadata = serde_json::from_slice(
        body.get(4..4 + length)
            .ok_or("Truncated recovery metadata")?,
    )
    .map_err(|error| error.to_string())?;
    if !metadata.unsaved {
        return Ok(None);
    }
    format::decode_plan(&body[4 + length..])
        .map(|plan| Some((plan, metadata.source)))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{load, save};
    use crate::model::PlanFile;
    #[test]
    fn clean_session_never_restores_and_dirty_session_keeps_source() {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("autosave.rpp");
        let source = dir.path().join("original.rpp");
        save(&path, &PlanFile::default(), &[], Some(&source), true).expect("dirty recovery");
        assert_eq!(
            load(&path).expect("load").expect("unsaved session").1,
            Some(source.clone())
        );
        save(&path, &PlanFile::default(), &[], Some(&source), false).expect("clean marker");
        assert!(load(&path).expect("load clean").is_none());
    }
    #[test]
    fn incomplete_envelope_is_rejected() {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("autosave.rpp");
        std::fs::write(&path, super::HEADER).expect("write");
        assert!(load(&path).is_err());
    }
}
