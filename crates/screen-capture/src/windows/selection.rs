use super::{CaptureError, NormalizedRect, Size, WindowHandle, client_size};
use crate::PixelRect;
use std::ffi::c_void;
use std::sync::OnceLock;

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct WindowClass {
    style: u32,
    procedure: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
#[derive(Default)]
struct Message {
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}

#[repr(C)]
#[derive(Default)]
struct Paint {
    dc: isize,
    erase: i32,
    rect: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetLastError() -> u32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const c_void,
    ) -> isize;
    fn DestroyWindow(window: isize) -> i32;
    fn ShowWindow(window: isize, command: i32) -> i32;
    fn SetForegroundWindow(window: isize) -> i32;
    fn SetFocus(window: isize) -> isize;
    fn SetLayeredWindowAttributes(window: isize, key: u32, alpha: u8, flags: u32) -> i32;
    fn ClientToScreen(window: isize, point: *mut Point) -> i32;
    fn SetWindowLongPtrW(window: isize, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(window: isize, index: i32) -> isize;
    fn DefWindowProcW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn GetMessageW(message: *mut Message, window: isize, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn SetCapture(window: isize) -> isize;
    fn ReleaseCapture() -> i32;
    fn InvalidateRect(window: isize, rect: *const Rect, erase: i32) -> i32;
    fn BeginPaint(window: isize, paint: *mut Paint) -> isize;
    fn EndPaint(window: isize, paint: *const Paint) -> i32;
    fn FillRect(dc: isize, rect: *const Rect, brush: isize) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetStockObject(index: i32) -> isize;
    fn SelectObject(dc: isize, object: isize) -> isize;
    fn Rectangle(dc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn SetTextColor(dc: isize, color: u32) -> u32;
    fn SetBkMode(dc: isize, mode: i32) -> i32;
    fn TextOutW(dc: isize, x: i32, y: i32, text: *const u16, count: i32) -> i32;
}

const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const GWLP_USERDATA: i32 = -21;

struct State {
    bounds: Size,
    prompt: Vec<u16>,
    start: Option<(i32, i32)>,
    current: Option<(i32, i32)>,
    selected: Option<PixelRect>,
    cancelled: bool,
}

fn mouse_point(lparam: isize, bounds: Size) -> (i32, i32) {
    let x = (lparam as u16 as i16 as i32).clamp(0, bounds.width as i32);
    let y = ((lparam >> 16) as u16 as i16 as i32).clamp(0, bounds.height as i32);
    (x, y)
}

unsafe extern "system" fn window_proc(
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) };
    if pointer == 0 {
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    // SAFETY: pointer belongs to the stack-owned State and is cleared before destruction.
    let state = unsafe { &mut *(pointer as *mut State) };
    match message {
        WM_LBUTTONDOWN => {
            let point = mouse_point(lparam, state.bounds);
            state.start = Some(point);
            state.current = Some(point);
            unsafe {
                SetCapture(window);
                InvalidateRect(window, std::ptr::null(), 0);
            }
            0
        }
        WM_MOUSEMOVE => {
            if state.start.is_some() {
                state.current = Some(mouse_point(lparam, state.bounds));
                unsafe {
                    InvalidateRect(window, std::ptr::null(), 0);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            unsafe {
                ReleaseCapture();
            }
            if let Some((start_x, start_y)) = state.start.take() {
                let (end_x, end_y) = mouse_point(lparam, state.bounds);
                let region = PixelRect {
                    left: start_x.min(end_x),
                    top: start_y.min(end_y),
                    right: start_x.max(end_x),
                    bottom: start_y.max(end_y),
                };
                if region.size().is_ok() {
                    state.selected = Some(region);
                }
            }
            state.current = None;
            unsafe {
                InvalidateRect(window, std::ptr::null(), 0);
            }
            0
        }
        WM_KEYDOWN if wparam == 27 => {
            state.cancelled = true;
            0
        }
        WM_CLOSE => {
            state.cancelled = true;
            0
        }
        WM_PAINT => {
            let mut paint = Paint::default();
            let dc = unsafe { BeginPaint(window, &mut paint) };
            if dc != 0 {
                let all = Rect {
                    left: 0,
                    top: 0,
                    right: state.bounds.width as i32,
                    bottom: state.bounds.height as i32,
                };
                unsafe {
                    FillRect(dc, &all, GetStockObject(4)); // BLACK_BRUSH
                    SetTextColor(dc, 0x00FF_FFFF);
                    SetBkMode(dc, 1); // TRANSPARENT
                    TextOutW(dc, 16, 16, state.prompt.as_ptr(), state.prompt.len() as i32);
                }
                if let (Some((start_x, start_y)), Some((end_x, end_y))) =
                    (state.start, state.current)
                {
                    let old_brush = unsafe { SelectObject(dc, GetStockObject(5)) }; // NULL_BRUSH
                    let old_pen = unsafe { SelectObject(dc, GetStockObject(6)) }; // WHITE_PEN
                    unsafe {
                        Rectangle(
                            dc,
                            start_x.min(end_x),
                            start_y.min(end_y),
                            start_x.max(end_x),
                            start_y.max(end_y),
                        );
                        SelectObject(dc, old_pen);
                        SelectObject(dc, old_brush);
                    }
                }
                unsafe {
                    EndPaint(window, &paint);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn ensure_class(class_name: &[u16]) -> Result<(), CaptureError> {
    static REGISTERED: OnceLock<Result<(), CaptureError>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let class = WindowClass {
                style: 3,
                procedure: Some(window_proc),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: 0,
                menu_name: std::ptr::null(),
                class_name: class_name.as_ptr(),
            };
            let atom = unsafe { RegisterClassW(&class) };
            if atom == 0 && unsafe { GetLastError() } != 1410 {
                // ERROR_CLASS_ALREADY_EXISTS
                Err(CaptureError::WindowsApi("RegisterClassW"))
            } else {
                Ok(())
            }
        })
        .clone()
}

pub(crate) fn select_region(
    target: WindowHandle,
    prompt: &str,
) -> Result<NormalizedRect, CaptureError> {
    let bounds = client_size(target)?;
    let mut origin = Point::default();
    if unsafe { ClientToScreen(target.0, &mut origin) } == 0 {
        return Err(CaptureError::WindowsApi("ClientToScreen"));
    }
    let class_name = wide("MSHCalibrationOverlay");
    ensure_class(&class_name)?;
    let title = wide(&format!("Mahjong Screen Helper Calibration: {prompt}"));
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    const WS_EX_TOPMOST: u32 = 0x0000_0008;
    const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
    const WS_EX_LAYERED: u32 = 0x0008_0000;
    const WS_POPUP: u32 = 0x8000_0000;
    let overlay = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            origin.x,
            origin.y,
            bounds.width as i32,
            bounds.height as i32,
            0,
            0,
            instance,
            std::ptr::null(),
        )
    };
    if overlay == 0 {
        return Err(CaptureError::WindowsApi("CreateWindowExW"));
    }
    let prompt = if prompt.is_empty() {
        "拖动框选区域，Esc 取消"
    } else {
        prompt
    };
    let mut state = State {
        bounds,
        prompt: prompt.encode_utf16().collect(),
        start: None,
        current: None,
        selected: None,
        cancelled: false,
    };
    unsafe {
        SetWindowLongPtrW(overlay, GWLP_USERDATA, &mut state as *mut State as isize);
        SetLayeredWindowAttributes(overlay, 0, 110, 2); // LWA_ALPHA
        ShowWindow(overlay, 5); // SW_SHOW
        SetForegroundWindow(overlay);
        SetFocus(overlay);
        InvalidateRect(overlay, std::ptr::null(), 1);
    }
    let mut error = None;
    while !state.cancelled && state.selected.is_none() {
        let mut message = Message::default();
        let result = unsafe { GetMessageW(&mut message, overlay, 0, 0) };
        if result <= 0 {
            error = Some(CaptureError::WindowsApi("GetMessageW"));
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unsafe {
        SetWindowLongPtrW(overlay, GWLP_USERDATA, 0);
        DestroyWindow(overlay);
    }
    if let Some(error) = error {
        return Err(error);
    }
    if state.cancelled {
        return Err(CaptureError::Cancelled);
    }
    NormalizedRect::from_pixels(bounds, state.selected.ok_or(CaptureError::Cancelled)?)
}
