//! Read-only capture of a calibrated Windows client area.

use serde::{Deserialize, Serialize};
use std::fmt;

mod profile;
pub use profile::{CalibrationProfile, RegionKind, calibrate_window};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PixelRect {
    pub fn size(self) -> Result<Size, CaptureError> {
        if self.left < 0 || self.top < 0 || self.right <= self.left || self.bottom <= self.top {
            return Err(CaptureError::InvalidRegion);
        }
        Ok(Size {
            width: (self.right - self.left) as u32,
            height: (self.bottom - self.top) as u32,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizedRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl NormalizedRect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Result<Self, CaptureError> {
        if [left, top, right, bottom]
            .iter()
            .any(|value| !value.is_finite())
            || left < 0.0
            || top < 0.0
            || right > 1.0
            || bottom > 1.0
            || right <= left
            || bottom <= top
        {
            return Err(CaptureError::InvalidRegion);
        }
        Ok(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    pub fn from_pixels(client: Size, region: PixelRect) -> Result<Self, CaptureError> {
        if client.width == 0
            || client.height == 0
            || client.width > i32::MAX as u32
            || client.height > i32::MAX as u32
        {
            return Err(CaptureError::InvalidSize);
        }
        region.size()?;
        if region.right > client.width as i32 || region.bottom > client.height as i32 {
            return Err(CaptureError::InvalidRegion);
        }
        Self::new(
            region.left as f32 / client.width as f32,
            region.top as f32 / client.height as f32,
            region.right as f32 / client.width as f32,
            region.bottom as f32 / client.height as f32,
        )
    }

    pub fn resolve(self, client: Size) -> Result<PixelRect, CaptureError> {
        Self::new(self.left, self.top, self.right, self.bottom)?;
        if client.width == 0
            || client.height == 0
            || client.width > i32::MAX as u32
            || client.height > i32::MAX as u32
        {
            return Err(CaptureError::InvalidSize);
        }
        let width = client.width as f32;
        let height = client.height as f32;
        let region = PixelRect {
            left: (self.left * width).round() as i32,
            top: (self.top * height).round() as i32,
            right: (self.right * width).round() as i32,
            bottom: (self.bottom * height).round() as i32,
        };
        region.size()?;
        if region.right > client.width as i32 || region.bottom > client.height as i32 {
            return Err(CaptureError::InvalidRegion);
        }
        Ok(region)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    InvalidRegion,
    InvalidSize,
    InvalidWindow,
    EmptyTitleQuery,
    Cancelled,
    ConfigFormat,
    Io(String),
    Graphics(String),
    Timeout,
    Closed,
    AllocationTooLarge,
    WindowsApi(&'static str),
    UnsupportedPlatform,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CaptureError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub isize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub handle: WindowHandle,
    pub title: String,
    pub client: Size,
    pub dpi: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub source: WindowHandle,
    pub region: PixelRect,
    pub dpi: u32,
    pub size: Size,
    /// Top-down BGRA pixels, four bytes per pixel. Kept in memory only.
    pub bgra: Vec<u8>,
}

#[cfg(windows)]
mod wgc;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use wgc::GraphicsSession;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureBackend {
    GraphicsCapture,
    BitBlt,
}

pub struct CaptureStream {
    handle: WindowHandle,
    region: NormalizedRect,
    #[cfg(windows)]
    graphics: Option<GraphicsSession>,
}

impl CaptureStream {
    /// Prefer Windows Graphics Capture, using BitBlt when the graphics session
    /// cannot start or stops producing usable frames.
    pub fn start(handle: WindowHandle, region: NormalizedRect) -> Result<Self, CaptureError> {
        region.resolve({
            #[cfg(windows)]
            {
                windows::client_size(handle)?
            }
            #[cfg(not(windows))]
            {
                return Err(CaptureError::UnsupportedPlatform);
            }
        })?;
        #[cfg(windows)]
        let graphics = GraphicsSession::start(handle, region).ok();
        Ok(Self {
            handle,
            region,
            #[cfg(windows)]
            graphics,
        })
    }

    pub fn backend(&self) -> CaptureBackend {
        #[cfg(windows)]
        if self.graphics.is_some() {
            return CaptureBackend::GraphicsCapture;
        }
        CaptureBackend::BitBlt
    }

    pub fn next_frame(&mut self, timeout: std::time::Duration) -> Result<Frame, CaptureError> {
        #[cfg(windows)]
        {
            if let Some(graphics) = self.graphics.as_ref() {
                match graphics.next_frame(timeout) {
                    Ok(frame) => return Ok(frame),
                    Err(CaptureError::Timeout) => return Err(CaptureError::Timeout),
                    Err(_) => self.graphics = None,
                }
            }
            windows::capture_region(self.handle, self.region)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err(CaptureError::UnsupportedPlatform)
        }
    }
}

/// Enumerate visible top-level windows whose title contains `query`.
pub fn find_windows(query: &str) -> Result<Vec<WindowInfo>, CaptureError> {
    if query.trim().is_empty() {
        return Err(CaptureError::EmptyTitleQuery);
    }
    #[cfg(windows)]
    {
        windows::find_windows(query)
    }
    #[cfg(not(windows))]
    {
        let _ = query;
        Err(CaptureError::UnsupportedPlatform)
    }
}

/// Resolve the current client size each time, so moving or resizing the target
/// window does not invalidate a saved normalized region.
pub fn capture_region(handle: WindowHandle, region: NormalizedRect) -> Result<Frame, CaptureError> {
    #[cfg(windows)]
    {
        windows::capture_region(handle, region)
    }
    #[cfg(not(windows))]
    {
        let _ = (handle, region);
        Err(CaptureError::UnsupportedPlatform)
    }
}

/// Open a temporary translucent window over the target client area. Drag to
/// select a region; press Escape to cancel. The window never writes a frame to disk.
pub fn select_region(handle: WindowHandle, prompt: &str) -> Result<NormalizedRect, CaptureError> {
    #[cfg(windows)]
    {
        windows::select_region(handle, prompt)
    }
    #[cfg(not(windows))]
    {
        let _ = (handle, prompt);
        Err(CaptureError::UnsupportedPlatform)
    }
}
