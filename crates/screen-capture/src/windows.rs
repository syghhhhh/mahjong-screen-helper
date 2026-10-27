use super::{CaptureError, Frame, NormalizedRect, Size, WindowHandle, WindowInfo};
use std::ffi::c_void;

mod selection;
pub(super) use selection::select_region;

#[repr(C)]
#[derive(Default)]
struct RawRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pixels_per_meter: i32,
    y_pixels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        data: isize,
    ) -> i32;
    fn IsWindow(window: isize) -> i32;
    fn IsWindowVisible(window: isize) -> i32;
    fn IsIconic(window: isize) -> i32;
    fn GetWindowTextLengthW(window: isize) -> i32;
    fn GetWindowTextW(window: isize, buffer: *mut u16, max_count: i32) -> i32;
    fn GetClientRect(window: isize, rect: *mut RawRect) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
    fn GetDC(window: isize) -> isize;
    fn ReleaseDC(window: isize, dc: isize) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: isize) -> isize;
    fn DeleteDC(dc: isize) -> i32;
    fn CreateCompatibleBitmap(dc: isize, width: i32, height: i32) -> isize;
    fn SelectObject(dc: isize, object: isize) -> isize;
    fn DeleteObject(object: isize) -> i32;
    fn BitBlt(
        destination: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        source: isize,
        source_x: i32,
        source_y: i32,
        operation: u32,
    ) -> i32;
    fn GetDIBits(
        dc: isize,
        bitmap: isize,
        start: u32,
        lines: u32,
        bits: *mut c_void,
        info: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
}

pub(crate) fn client_size(handle: WindowHandle) -> Result<Size, CaptureError> {
    let mut rect = RawRect::default();
    // SAFETY: the caller supplies a Win32 handle; the API validates it and fills `rect`.
    if unsafe { IsWindow(handle.0) } == 0 || unsafe { GetClientRect(handle.0, &mut rect) } == 0 {
        return Err(CaptureError::InvalidWindow);
    }
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return Err(CaptureError::InvalidSize);
    }
    Ok(Size {
        width: (rect.right - rect.left) as u32,
        height: (rect.bottom - rect.top) as u32,
    })
}

struct SearchContext {
    query: String,
    found: Vec<WindowInfo>,
}

unsafe extern "system" fn visit_window(raw: isize, data: isize) -> i32 {
    // SAFETY: EnumWindows calls synchronously while SearchContext is alive.
    let context = unsafe { &mut *(data as *mut SearchContext) };
    if unsafe { IsWindowVisible(raw) } == 0 || unsafe { IsIconic(raw) } != 0 {
        return 1;
    }
    let length = unsafe { GetWindowTextLengthW(raw) };
    if length <= 0 {
        return 1;
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(raw, buffer.as_mut_ptr(), length + 1) };
    if written <= 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&buffer[..written as usize]);
    if !title.to_lowercase().contains(&context.query) {
        return 1;
    }
    let handle = WindowHandle(raw);
    if let Ok(client) = client_size(handle) {
        let dpi = unsafe { GetDpiForWindow(raw) };
        context.found.push(WindowInfo {
            handle,
            title,
            client,
            dpi: dpi.max(96),
        });
    }
    1
}

pub(super) fn find_windows(query: &str) -> Result<Vec<WindowInfo>, CaptureError> {
    let mut context = SearchContext {
        query: query.to_lowercase(),
        found: Vec::new(),
    };
    // SAFETY: callback and context remain valid for the synchronous enumeration.
    let ok = unsafe {
        EnumWindows(
            Some(visit_window),
            &mut context as *mut SearchContext as isize,
        )
    };
    if ok == 0 {
        return Err(CaptureError::WindowsApi("EnumWindows"));
    }
    Ok(context.found)
}

struct Resources {
    window: isize,
    source_dc: isize,
    memory_dc: isize,
    bitmap: isize,
    old_bitmap: isize,
}

impl Drop for Resources {
    fn drop(&mut self) {
        // SAFETY: all handles were obtained from Win32 in this call; each is released once.
        unsafe {
            if self.old_bitmap != 0 && self.memory_dc != 0 {
                SelectObject(self.memory_dc, self.old_bitmap);
            }
            if self.bitmap != 0 {
                DeleteObject(self.bitmap);
            }
            if self.memory_dc != 0 {
                DeleteDC(self.memory_dc);
            }
            if self.source_dc != 0 {
                ReleaseDC(self.window, self.source_dc);
            }
        }
    }
}

pub(super) fn capture_region(
    handle: WindowHandle,
    region: NormalizedRect,
) -> Result<Frame, CaptureError> {
    let client = client_size(handle)?;
    let rect = region.resolve(client)?;
    let size = rect.size()?;
    let byte_len = (size.width as usize)
        .checked_mul(size.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(CaptureError::AllocationTooLarge)?;
    if byte_len > u32::MAX as usize {
        return Err(CaptureError::AllocationTooLarge);
    }
    let mut bgra = vec![0_u8; byte_len];
    let dpi = unsafe { GetDpiForWindow(handle.0) }.max(96);
    // SAFETY: each Win32 handle is checked before use and owned by Resources.
    let source_dc = unsafe { GetDC(handle.0) };
    if source_dc == 0 {
        return Err(CaptureError::WindowsApi("GetDC"));
    }
    let mut resources = Resources {
        window: handle.0,
        source_dc,
        memory_dc: 0,
        bitmap: 0,
        old_bitmap: 0,
    };
    resources.memory_dc = unsafe { CreateCompatibleDC(source_dc) };
    if resources.memory_dc == 0 {
        return Err(CaptureError::WindowsApi("CreateCompatibleDC"));
    }
    resources.bitmap =
        unsafe { CreateCompatibleBitmap(source_dc, size.width as i32, size.height as i32) };
    if resources.bitmap == 0 {
        return Err(CaptureError::WindowsApi("CreateCompatibleBitmap"));
    }
    resources.old_bitmap = unsafe { SelectObject(resources.memory_dc, resources.bitmap) };
    if resources.old_bitmap == 0 || resources.old_bitmap == -1 {
        resources.old_bitmap = 0;
        return Err(CaptureError::WindowsApi("SelectObject"));
    }
    const SRCCOPY: u32 = 0x00CC_0020;
    let copied = unsafe {
        BitBlt(
            resources.memory_dc,
            0,
            0,
            size.width as i32,
            size.height as i32,
            source_dc,
            rect.left,
            rect.top,
            SRCCOPY,
        )
    };
    if copied == 0 {
        return Err(CaptureError::WindowsApi("BitBlt"));
    }
    let restored = unsafe { SelectObject(resources.memory_dc, resources.old_bitmap) };
    if restored == 0 || restored == -1 {
        return Err(CaptureError::WindowsApi("RestoreBitmap"));
    }
    resources.old_bitmap = 0;
    let mut info = BitmapInfo {
        header: BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: size.width as i32,
            height: -(size.height as i32),
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: byte_len as u32,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        },
        colors: [0],
    };
    let lines = unsafe {
        GetDIBits(
            source_dc,
            resources.bitmap,
            0,
            size.height,
            bgra.as_mut_ptr().cast(),
            &mut info,
            0,
        )
    };
    if lines != size.height as i32 {
        return Err(CaptureError::WindowsApi("GetDIBits"));
    }
    Ok(Frame {
        client,
        source: handle,
        region: rect,
        dpi,
        size,
        bgra,
    })
}
