use super::{CaptureError, NormalizedRect, Size, WindowInfo, select_region};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionKind {
    Hand,
    Melds,
    River,
    Dora,
    Actions,
}

impl RegionKind {
    pub const ALL: [Self; 5] = [
        Self::Hand,
        Self::Melds,
        Self::River,
        Self::Dora,
        Self::Actions,
    ];

    fn prompt(self) -> &'static str {
        match self {
            Self::Hand => "框选自己的手牌区域；拖动鼠标，Esc 取消",
            Self::Melds => "框选自己的副露区域；拖动鼠标，Esc 取消",
            Self::River => "框选可见牌河区域；拖动鼠标，Esc 取消",
            Self::Dora => "框选宝牌指示牌区域；拖动鼠标，Esc 取消",
            Self::Actions => "框选行动按钮区域；拖动鼠标，Esc 取消",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationProfile {
    pub schema_version: u32,
    pub window_title_query: String,
    pub reference_client: Size,
    pub reference_dpi: u32,
    pub regions: BTreeMap<RegionKind, NormalizedRect>,
}

impl CalibrationProfile {
    pub fn new(
        window_title_query: impl Into<String>,
        reference_client: Size,
        reference_dpi: u32,
    ) -> Result<Self, CaptureError> {
        let profile = Self {
            schema_version: 1,
            window_title_query: window_title_query.into(),
            reference_client,
            reference_dpi,
            regions: BTreeMap::new(),
        };
        profile.validate(false)?;
        Ok(profile)
    }

    pub fn set_region(
        &mut self,
        kind: RegionKind,
        region: NormalizedRect,
    ) -> Result<(), CaptureError> {
        NormalizedRect::new(region.left, region.top, region.right, region.bottom)?;
        region.resolve(self.reference_client)?;
        self.regions.insert(kind, region);
        Ok(())
    }

    pub fn region(&self, kind: RegionKind) -> Option<NormalizedRect> {
        self.regions.get(&kind).copied()
    }

    pub fn is_complete(&self) -> bool {
        RegionKind::ALL
            .iter()
            .all(|kind| self.regions.contains_key(kind))
    }

    fn validate(&self, require_complete: bool) -> Result<(), CaptureError> {
        if self.schema_version != 1
            || self.window_title_query.trim().is_empty()
            || self.window_title_query.len() > 512
            || self.reference_client.width == 0
            || self.reference_client.height == 0
            || self.reference_dpi == 0
            || self.reference_dpi > 1000
            || (require_complete && !self.is_complete())
        {
            return Err(CaptureError::ConfigFormat);
        }
        for region in self.regions.values() {
            NormalizedRect::new(region.left, region.top, region.right, region.bottom)?;
            region.resolve(self.reference_client)?;
        }
        Ok(())
    }

    /// Writes only the window query and normalized coordinates; image data is never serialized.
    pub fn save(&self, path: &Path) -> Result<(), CaptureError> {
        self.validate(true)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| CaptureError::Io(error.to_string()))?;
        }
        let content = serde_json::to_vec_pretty(self).map_err(|_| CaptureError::ConfigFormat)?;
        std::fs::write(path, content).map_err(|error| CaptureError::Io(error.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, CaptureError> {
        let content = std::fs::read(path).map_err(|error| CaptureError::Io(error.to_string()))?;
        let profile: Self =
            serde_json::from_slice(&content).map_err(|_| CaptureError::ConfigFormat)?;
        profile.validate(true)?;
        Ok(profile)
    }
}

/// First-run calibration: the user drags five regions over the selected target window.
pub fn calibrate_window(
    window: &WindowInfo,
    query: &str,
) -> Result<CalibrationProfile, CaptureError> {
    let mut profile = CalibrationProfile::new(query, window.client, window.dpi)?;
    for kind in RegionKind::ALL {
        let selected = select_region(window.handle, kind.prompt())?;
        profile.set_region(kind, selected)?;
    }
    Ok(profile)
}
