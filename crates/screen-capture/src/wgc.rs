//! Windows Graphics Capture session. It keeps only the latest cropped frame in memory.

use crate::{CaptureError, Frame, NormalizedRect, Size, WindowHandle};
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as GraphicsFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Rect {
    fn size(self) -> Size {
        Size {
            width: (self.right - self.left) as u32,
            height: (self.bottom - self.top) as u32,
        }
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetWindowRect(window: isize, rect: *mut Rect) -> i32;
    fn ClientToScreen(window: isize, point: *mut Point) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmGetWindowAttribute(window: isize, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

struct Latest {
    frame: Option<Result<Frame, CaptureError>>,
    closed: bool,
}

struct Shared {
    latest: Mutex<Latest>,
    ready: Condvar,
}

#[derive(Clone)]
struct Flags {
    shared: Arc<Shared>,
    handle: WindowHandle,
    region: NormalizedRect,
}

struct Handler {
    flags: Flags,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Flags;
    type Error = CaptureError;

    fn new(context: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            flags: context.flags,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut GraphicsFrame<'_>,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let result = crop_client_frame(frame, self.flags.handle, self.flags.region);
        let mut latest = self
            .flags
            .shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        latest.frame = Some(result);
        self.flags.shared.ready.notify_one();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        let mut latest = self
            .flags
            .shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        latest.closed = true;
        self.flags.shared.ready.notify_all();
        Ok(())
    }
}

pub struct GraphicsSession {
    shared: Arc<Shared>,
    control: Option<CaptureControl<Handler, CaptureError>>,
}

impl GraphicsSession {
    pub fn start(handle: WindowHandle, region: NormalizedRect) -> Result<Self, CaptureError> {
        crate::windows::client_size(handle)?;
        let shared = Arc::new(Shared {
            latest: Mutex::new(Latest {
                frame: None,
                closed: false,
            }),
            ready: Condvar::new(),
        });
        let flags = Flags {
            shared: Arc::clone(&shared),
            handle,
            region,
        };
        let window = Window::from_raw_hwnd(handle.0 as *mut c_void);
        let settings = Settings::new(
            window,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Exclude,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            flags,
        );
        let control = Handler::start_free_threaded(settings)
            .map_err(|error| CaptureError::Graphics(error.to_string()))?;
        Ok(Self {
            shared,
            control: Some(control),
        })
    }

    /// Wait for the newest frame. Incoming frames replace older unread frames.
    pub fn next_frame(&self, timeout: Duration) -> Result<Frame, CaptureError> {
        let latest = self
            .shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let (mut latest, _) = self
            .shared
            .ready
            .wait_timeout_while(latest, timeout, |state| {
                state.frame.is_none() && !state.closed
            })
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(frame) = latest.frame.take() {
            return frame;
        }
        if latest.closed
            || self
                .control
                .as_ref()
                .is_some_and(|control| control.is_finished())
        {
            Err(CaptureError::Closed)
        } else {
            Err(CaptureError::Timeout)
        }
    }

    pub fn stop(mut self) -> Result<(), CaptureError> {
        if let Some(control) = self.control.take() {
            control
                .stop()
                .map_err(|error| CaptureError::Graphics(error.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for GraphicsSession {
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            let _ = control.stop();
        }
    }
}

fn capture_bounds(
    handle: WindowHandle,
    frame: &GraphicsFrame<'_>,
    client: Size,
    origin: Point,
) -> Result<Rect, CaptureError> {
    let mut outer = Rect::default();
    if unsafe { GetWindowRect(handle.0, &mut outer) } == 0 {
        return Err(CaptureError::Graphics("GetWindowRect failed".into()));
    }
    let mut extended = Rect::default();
    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
    let extended_ok = unsafe {
        DwmGetWindowAttribute(
            handle.0,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut extended as *mut Rect).cast(),
            std::mem::size_of::<Rect>() as u32,
        )
    } == 0;
    let frame_size = Size {
        width: frame.width(),
        height: frame.height(),
    };
    if extended_ok && extended.size() == frame_size {
        return Ok(extended);
    }
    if outer.size() == frame_size {
        return Ok(outer);
    }
    if client == frame_size {
        return Ok(Rect {
            left: origin.x,
            top: origin.y,
            right: origin.x + client.width as i32,
            bottom: origin.y + client.height as i32,
        });
    }
    Err(CaptureError::Graphics(format!(
        "Cannot align capture frame {}x{} with target window",
        frame.width(),
        frame.height()
    )))
}

fn crop_client_frame(
    frame: &mut GraphicsFrame<'_>,
    handle: WindowHandle,
    region: NormalizedRect,
) -> Result<Frame, CaptureError> {
    let client = crate::windows::client_size(handle)?;
    let client_region = region.resolve(client)?;
    let mut origin = Point::default();
    if unsafe { ClientToScreen(handle.0, &mut origin) } == 0 {
        return Err(CaptureError::Graphics("ClientToScreen failed".into()));
    }
    let bounds = capture_bounds(handle, frame, client, origin)?;
    let left = origin.x - bounds.left + client_region.left;
    let top = origin.y - bounds.top + client_region.top;
    let right = origin.x - bounds.left + client_region.right;
    let bottom = origin.y - bounds.top + client_region.bottom;
    if left < 0 || top < 0 || right > frame.width() as i32 || bottom > frame.height() as i32 {
        return Err(CaptureError::Graphics(
            "Calibrated region falls outside graphics frame".into(),
        ));
    }
    let cropped = frame
        .buffer_crop(left as u32, top as u32, right as u32, bottom as u32)
        .map_err(|error| CaptureError::Graphics(error.to_string()))?;
    let mut scratch = Vec::new();
    let bgra = cropped.as_nopadding_buffer(&mut scratch).to_vec();
    let dpi = unsafe { GetDpiForWindow(handle.0) }.max(96);
    let size = client_region.size()?;
    Ok(Frame {
        source: handle,
        region: client_region,
        dpi,
        size,
        bgra,
    })
}
