use game_state::GameTracker;
use riichi_core::Tile;
use runtime_core::{PipelineOutput, PublicImages, RuntimeEngine};
use screen_capture::{
    CalibrationProfile, CaptureError, CaptureStream, RegionKind, WindowInfo, calibrate_window,
    capture_region, find_windows, select_region,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::time::Duration;
use vision_core::{MatchConfig, RgbImage, TemplateBank, save_labeled_hand};

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
    fn DefWindowProcW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(window: isize) -> i32;
    fn ShowWindow(window: isize, command: i32) -> i32;
    fn UpdateWindow(window: isize) -> i32;
    fn GetMessageW(message: *mut Message, window: isize, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn BeginPaint(window: isize, paint: *mut Paint) -> isize;
    fn EndPaint(window: isize, paint: *const Paint) -> i32;
    fn FillRect(dc: isize, rect: *const Rect, brush: isize) -> i32;
    fn GetClientRect(window: isize, rect: *mut Rect) -> i32;
    fn GetWindowRect(window: isize, rect: *mut Rect) -> i32;
    fn InvalidateRect(window: isize, rect: *const Rect, erase: i32) -> i32;
    fn SetLayeredWindowAttributes(window: isize, key: u32, alpha: u8, flags: u32) -> i32;
    fn SetTimer(window: isize, id: usize, interval_ms: u32, callback: *const c_void) -> usize;
    fn KillTimer(window: isize, id: usize) -> i32;
    fn RegisterHotKey(window: isize, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(window: isize, id: i32) -> i32;
    fn GetWindowTextW(window: isize, buffer: *mut u16, max: i32) -> i32;
    fn SetWindowTextW(window: isize, text: *const u16) -> i32;
    fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn DeleteObject(object: isize) -> i32;
    fn GetStockObject(index: i32) -> isize;
    fn SelectObject(dc: isize, object: isize) -> isize;
    fn SetTextColor(dc: isize, color: u32) -> u32;
    fn SetBkMode(dc: isize, mode: i32) -> i32;
    fn TextOutW(dc: isize, x: i32, y: i32, text: *const u16, count: i32) -> i32;
}

const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_THICKFRAME: u32 = 0x0004_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const LWA_ALPHA: u32 = 0x0000_0002;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000f;
const WM_CLOSE: u32 = 0x0010;
const WM_TIMER: u32 = 0x0113;
const WM_HOTKEY: u32 = 0x0312;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_NCHITTEST: u32 = 0x0084;
const HTCAPTION: isize = 2;
const HTBOTTOMRIGHT: isize = 17;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_F8: u32 = 0x77;
const VK_F9: u32 = 0x78;
const VK_F10: u32 = 0x79;

thread_local! { static APP: RefCell<Option<AppState>> = const { RefCell::new(None) }; }

struct AppState {
    window: isize,
    edit: isize,
    selected: Option<WindowInfo>,
    active_query: String,
    profile: Option<CalibrationProfile>,
    config_path: PathBuf,
    pack_path: PathBuf,
    runtime: Option<RuntimeEngine>,
    manual_tracker: GameTracker,
    capture_stream: Option<CaptureStream>,
    last_images: BTreeMap<RegionKind, RgbImage>,
    last_output: Option<PipelineOutput>,
    frame_id: u64,
    manual_override: bool,
    show_hotkey: bool,
    hidden: bool,
    selection_cursor: usize,
    smoke_failed: bool,
    status: String,
}

#[derive(Clone, Copy)]
enum AppAction {
    Find,
    Calibrate,
    Start,
    Pause,
    Correct,
    Learn,
    Refresh,
    Hide,
    Quit,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn color(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}

pub fn show_error(error: &str) {
    let text = wide(error);
    let title = wide("Riichi Screen Helper 启动失败");
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0x10);
    }
}

pub fn run(smoke: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let base = exe.parent().ok_or("无法定位程序目录")?;
    let config_path = base.join("config").join("calibration.json");
    let pack_path = base.join("models").join("template-pack");
    let profile = CalibrationProfile::load(&config_path).ok();
    let active_query = profile
        .as_ref()
        .map_or("雀魂".to_string(), |item| item.window_title_query.clone());
    APP.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            window: 0,
            edit: 0,
            selected: None,
            active_query,
            profile,
            config_path,
            pack_path,
            runtime: None,
            manual_tracker: GameTracker::new(),
            capture_stream: None,
            last_images: BTreeMap::new(),
            last_output: None,
            frame_id: 0,
            manual_override: false,
            show_hotkey: false,
            hidden: false,
            selection_cursor: 0,
            smoke_failed: false,
            status: "点击“找窗口”开始；首次使用请校准并准备本机模板包".into(),
        });
    });
    let class_name = wide("RiichiScreenHelperOverlayV001");
    let title = wide("Riichi Screen Helper");
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance == 0 {
        return Err("GetModuleHandleW 失败".into());
    }
    let class = WindowClass {
        style: 0,
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
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("RegisterClassW 失败".into());
    }
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_THICKFRAME | if smoke { 0 } else { WS_VISIBLE },
            90,
            90,
            470,
            330,
            0,
            0,
            instance,
            std::ptr::null(),
        )
    };
    if window == 0 {
        return Err("CreateWindowExW 失败".into());
    }
    let edit_class = wide("EDIT");
    let empty = wide("");
    let edit = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
            12,
            128,
            350,
            25,
            window,
            101,
            instance,
            std::ptr::null(),
        )
    };
    if edit == 0 {
        unsafe {
            DestroyWindow(window);
        }
        return Err("创建手牌修正输入框失败".into());
    }
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.window = window;
            state.edit = edit;
        }
    });
    unsafe {
        SetLayeredWindowAttributes(window, 0, 250, LWA_ALPHA);
        SetTimer(window, 1, 50, std::ptr::null());
        if smoke {
            let sample = wide("1m 2m 3m 4m 5m 6m 2p 3p 4p 2s 3s 7z 7z 9p");
            SetWindowTextW(edit, sample.as_ptr());
            PostMessageW(window, WM_LBUTTONUP, 0, (105_i32 << 16 | 60) as isize);
            SetTimer(window, 2, 250, std::ptr::null());
        }
        let show_hotkey = RegisterHotKey(window, 2, MOD_NOREPEAT, VK_F9) != 0;
        RegisterHotKey(window, 1, MOD_NOREPEAT, VK_F8);
        RegisterHotKey(window, 3, MOD_NOREPEAT, VK_F10);
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.show_hotkey = show_hotkey;
            }
        });
        if !smoke {
            ShowWindow(window, SW_SHOW);
            UpdateWindow(window);
        }
    }
    let mut message = Message::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, 0, 0, 0) };
        if result < 0 {
            return Err("GetMessageW 失败".into());
        }
        if result == 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let failed = APP.with(|cell| {
        let failed = cell
            .borrow()
            .as_ref()
            .is_some_and(|state| state.smoke_failed);
        *cell.borrow_mut() = None;
        failed
    });
    if failed {
        Err("桌面浮窗自测未显示 Top-3".into())
    } else {
        Ok(())
    }
}

unsafe extern "system" fn window_proc(
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_NCHITTEST => {
            let mut bounds = Rect::default();
            unsafe {
                GetWindowRect(window, &mut bounds);
            }
            let x = (lparam as u32 as u16 as i16) as i32 - bounds.left;
            let y = ((lparam as u32 >> 16) as u16 as i16) as i32 - bounds.top;
            if x >= bounds.right - bounds.left - 20 && y >= bounds.bottom - bounds.top - 20 {
                return HTBOTTOMRIGHT;
            }
            if (0..36).contains(&y) {
                return HTCAPTION;
            }
        }
        WM_PAINT => {
            paint(window);
            return 0;
        }
        WM_TIMER => {
            if wparam == 2 {
                APP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        state.smoke_failed = !matches!(state.last_output.as_ref(), Some(PipelineOutput::Ready { hand, discards, .. }) if hand.len() == 14 && discards.len() == 3);
                    }
                });
                unsafe {
                    PostMessageW(window, WM_CLOSE, 0, 0);
                }
            } else if wparam == 1 {
                let changed =
                    APP.with(|cell| cell.borrow_mut().as_mut().is_some_and(AppState::tick));
                if changed {
                    unsafe {
                        InvalidateRect(window, std::ptr::null(), 0);
                    }
                }
            }
            return 0;
        }
        WM_LBUTTONUP => {
            let x = lparam as u32 as u16 as i16 as i32;
            let y = (lparam as u32 >> 16) as u16 as i16 as i32;
            if let Some(action) = click_action(x, y) {
                run_action(action);
            }
            unsafe {
                InvalidateRect(window, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_HOTKEY => {
            match wparam {
                1 => run_action(AppAction::Pause),
                2 => run_action(AppAction::Hide),
                3 => run_action(AppAction::Refresh),
                _ => {}
            }
            unsafe {
                InvalidateRect(window, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(window);
            }
            return 0;
        }
        WM_DESTROY => {
            unsafe {
                KillTimer(window, 1);
                KillTimer(window, 2);
                for id in 1..=3 {
                    UnregisterHotKey(window, id);
                }
                PostQuitMessage(0);
            }
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn click_action(x: i32, y: i32) -> Option<AppAction> {
    if (370..458).contains(&x) && (128..153).contains(&y) {
        return Some(AppAction::Learn);
    }
    let row = if (48..82).contains(&y) {
        0
    } else if (88..122).contains(&y) {
        1
    } else {
        return None;
    };
    let column = if (12..119).contains(&x) {
        0
    } else if (125..232).contains(&x) {
        1
    } else if (238..345).contains(&x) {
        2
    } else if (351..458).contains(&x) {
        3
    } else {
        return None;
    };
    Some(match (row, column) {
        (0, 0) => AppAction::Find,
        (0, 1) => AppAction::Calibrate,
        (0, 2) => AppAction::Start,
        (0, 3) => AppAction::Pause,
        (1, 0) => AppAction::Correct,
        (1, 1) => AppAction::Refresh,
        (1, 2) => AppAction::Hide,
        _ => AppAction::Quit,
    })
}

fn run_action(action: AppAction) {
    match action {
        AppAction::Find => APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.find_target();
            }
        }),
        AppAction::Calibrate => calibrate_target(),
        AppAction::Start => APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.start();
            }
        }),
        AppAction::Pause => APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.toggle_pause();
            }
        }),
        AppAction::Correct => APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.correct_hand();
            }
        }),
        AppAction::Learn => learn_template(),
        AppAction::Refresh => APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.refresh();
            }
        }),
        AppAction::Hide => toggle_hidden(),
        AppAction::Quit => APP.with(|cell| {
            if let Some(state) = cell.borrow().as_ref() {
                unsafe {
                    PostMessageW(state.window, WM_CLOSE, 0, 0);
                }
            }
        }),
    }
}

fn toggle_hidden() {
    let state = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|state| (state.window, state.hidden, state.show_hotkey))
    });
    let Some((window, hidden, registered)) = state else {
        return;
    };
    if !registered {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.status = "F9 全局热键不可用，不能隐藏浮窗".into();
            }
        });
        return;
    }
    unsafe {
        ShowWindow(window, if hidden { SW_SHOW } else { SW_HIDE });
    }
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.hidden = !hidden;
        }
    });
}

fn calibrate_target() {
    let values = APP.with(|cell| {
        cell.borrow().as_ref().and_then(|state| {
            state.selected.clone().map(|window| {
                (
                    window,
                    state.window,
                    state.config_path.clone(),
                    state.query().to_string(),
                )
            })
        })
    });
    let Some((target, overlay, path, query)) = values else {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.status = "请先点击“找窗口”".into();
            }
        });
        return;
    };
    unsafe {
        ShowWindow(overlay, SW_HIDE);
    }
    let result = calibrate_window(&target, &query).and_then(|profile| {
        profile.save(&path)?;
        Ok(profile)
    });
    unsafe {
        ShowWindow(overlay, SW_SHOW);
        InvalidateRect(overlay, std::ptr::null(), 0);
    }
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            match result {
                Ok(profile) => {
                    state.profile = Some(profile);
                    state.runtime = None;
                    state.capture_stream = None;
                    state.last_images.clear();
                    state.last_output = None;
                    state.status = "八个区域已校准并保存".into();
                }
                Err(CaptureError::Cancelled) => state.status = "校准已取消".into(),
                Err(error) => state.status = format!("校准失败：{error}"),
            }
        }
    });
}

fn learn_template() {
    let values = APP.with(|cell| {
        cell.borrow().as_ref().and_then(|state| {
            state.selected.as_ref().map(|target| {
                (
                    target.handle,
                    state
                        .profile
                        .as_ref()
                        .and_then(|profile| profile.region(RegionKind::Hand)),
                    state.pack_path.clone(),
                    state.edit,
                    state.window,
                )
            })
        })
    });
    let Some((target, hand_region, pack_path, edit, overlay)) = values else {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.status = "请先找窗口，再学习本机模板".into();
            }
        });
        return;
    };
    let mut buffer = [0u16; 256];
    let count = unsafe { GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32) };
    let input = String::from_utf16_lossy(&buffer[..count.max(0) as usize]);
    if input.trim().is_empty() {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.status = "输入当前手牌牌码，或 @chi/@pon/@kan/@riichi/@none".into();
            }
        });
        return;
    }
    unsafe {
        ShowWindow(overlay, SW_HIDE);
    }
    let result: Result<String, String> = (|| {
        if let Some(label) = input.trim().strip_prefix('@') {
            let label = label.to_ascii_lowercase();
            let label = match label.as_str() {
                "none" | "chi" | "pon" | "kan" | "riichi" => label,
                _ => return Err("按钮标签应为 @chi/@pon/@kan/@riichi/@none".into()),
            };
            let prompt = format!("框选 {label} 按钮；@none 请框选无按钮背景");
            let region = select_region(target, &prompt).map_err(|error| error.to_string())?;
            let frame = capture_region(target, region).map_err(|error| error.to_string())?;
            let image = RgbImage::from_bgra(frame.size.width, frame.size.height, &frame.bgra)
                .map_err(|error| error.to_string())?;
            let directory = pack_path.join("actions");
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let path = directory.join(format!("{label}.png"));
            if path.exists() {
                return Err(format!("{label} 模板已存在；不会覆盖，请先核对本机文件"));
            }
            image.write_png(&path).map_err(|error| error.to_string())?;
            Ok(format!("已保存本机 {label} 按钮模板"))
        } else {
            let region = hand_region.ok_or("请先校准手牌区域")?;
            let tiles = input
                .split_whitespace()
                .map(Tile::from_code)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "牌码无效，请输入当前 13 或 14 张手牌")?;
            let frame = capture_region(target, region).map_err(|error| error.to_string())?;
            let image = RgbImage::from_bgra(frame.size.width, frame.size.height, &frame.bgra)
                .map_err(|error| error.to_string())?;
            let saved = save_labeled_hand(&image, &tiles, &pack_path)
                .map_err(|error| format!("手牌样本未保存：{error}"))?;
            Ok(format!("已保存 {saved} 张新牌图；已有模板不会覆盖"))
        }
    })();
    unsafe {
        ShowWindow(overlay, SW_SHOW);
        InvalidateRect(overlay, std::ptr::null(), 0);
    }
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.status = match result {
                Ok(message) => {
                    state.runtime = None;
                    state.capture_stream = None;
                    state.last_images.clear();
                    state.last_output = None;
                    state.manual_override = false;
                    format!("{message}；点击开始重新加载")
                }
                Err(message) => format!("学习模板：{message}"),
            };
        }
    });
}

impl AppState {
    fn query(&self) -> &str {
        &self.active_query
    }

    fn find_target(&mut self) {
        let query = self.query().to_string();
        let mut windows = find_windows(&query).unwrap_or_default();
        if windows.is_empty() && query != "Mahjong Soul" {
            windows = find_windows("Mahjong Soul").unwrap_or_default();
            if !windows.is_empty() {
                self.active_query = "Mahjong Soul".into();
            }
        }
        match windows {
            mut windows if !windows.is_empty() => {
                windows.sort_by(|a, b| a.title.cmp(&b.title));
                let index = self.selection_cursor % windows.len();
                self.selection_cursor += 1;
                let selected = windows.remove(index);
                self.status = format!("已选择：{}；点击校准或开始", selected.title);
                self.selected = Some(selected);
                self.runtime = None;
                self.capture_stream = None;
                self.last_images.clear();
                self.last_output = None;
            }
            _ => self.status = "未找到雀魂浏览器窗口；请打开对局页面".into(),
        }
    }

    fn start(&mut self) {
        self.runtime = None;
        self.capture_stream = None;
        self.last_images.clear();
        self.last_output = None;
        self.manual_override = false;
        let Some(target) = self.selected.as_ref() else {
            self.status = "请先找窗口".into();
            return;
        };
        let Some(profile) = self.profile.as_ref() else {
            self.status = "请先完成八区域校准".into();
            return;
        };
        let Ok(capture_bounds) = profile.capture_bounds() else {
            self.status = "请重新校准，补齐三家副露区域".into();
            return;
        };
        let bank = match TemplateBank::load_dir(&self.pack_path) {
            Ok(bank) => bank,
            Err(_) => {
                let progress = TemplateBank::progress_dir(&self.pack_path);
                self.status = format!(
                    "本机模板：牌图 {}/37、按钮 {}/5；可点击学习模板",
                    progress.tiles, progress.actions
                );
                return;
            }
        };
        let stream = match CaptureStream::start(target.handle, capture_bounds) {
            Ok(stream) => stream,
            Err(error) => {
                self.status = format!("采集启动失败：{error}");
                return;
            }
        };
        let runtime = match RuntimeEngine::new(bank, vec![], MatchConfig::default(), 3) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.status = format!("识别启动失败：{error}");
                return;
            }
        };
        self.status = format!(
            "只读识别运行中（{:?}）；按钮匹配待实图验收",
            stream.backend()
        );
        self.runtime = Some(runtime);
        self.capture_stream = Some(stream);
        self.last_images.clear();
        self.last_output = None;
        self.frame_id = 0;
        self.manual_override = false;
    }

    fn tick(&mut self) -> bool {
        if self.manual_override
            || self.runtime.is_none()
            || self.runtime.as_ref().is_some_and(RuntimeEngine::is_paused)
        {
            return false;
        }
        let Some(stream) = self.capture_stream.as_mut() else {
            return false;
        };
        let frame = match stream.next_frame(Duration::from_millis(10)) {
            Ok(frame) => frame,
            Err(CaptureError::Timeout) => return false,
            Err(error) => {
                self.status = format!("采集已停止：{error}");
                self.runtime = None;
                self.capture_stream = None;
                self.last_output = Some(PipelineOutput::Uncertain);
                return true;
            }
        };
        let Some(profile) = self.profile.as_ref() else {
            return false;
        };
        let mut images = BTreeMap::new();
        for kind in RegionKind::ALL {
            let result = profile
                .region(kind)
                .ok_or(CaptureError::InvalidRegion)
                .and_then(|region| frame.crop_normalized(region));
            let image = match result {
                Ok(crop) => RgbImage::from_bgra(crop.size.width, crop.size.height, &crop.bgra).ok(),
                Err(_) => None,
            };
            match image {
                Some(image) => {
                    images.insert(kind, image);
                }
                None => {
                    self.status = "区域裁剪失败，请重新校准".into();
                    self.runtime = None;
                    self.capture_stream = None;
                    self.last_output = Some(PipelineOutput::Uncertain);
                    return true;
                }
            }
        }
        if images == self.last_images
            && matches!(
                self.last_output.as_ref(),
                Some(PipelineOutput::Ready { .. })
            )
        {
            return false;
        }
        self.last_images = images;
        let images = &self.last_images;
        let public = PublicImages {
            river: &images[&RegionKind::River],
            own_melds: &images[&RegionKind::Melds],
            opponent_melds: [
                &images[&RegionKind::LeftMelds],
                &images[&RegionKind::AcrossMelds],
                &images[&RegionKind::RightMelds],
            ],
            dora: &images[&RegionKind::Dora],
        };
        self.frame_id = self.frame_id.saturating_add(1);
        let output = self
            .runtime
            .as_mut()
            .expect("checked above")
            .process_with_public(
                self.frame_id,
                &images[&RegionKind::Hand],
                Some(&images[&RegionKind::Actions]),
                &public,
            );
        self.last_output = Some(output);
        true
    }

    fn toggle_pause(&mut self) {
        let Some(runtime) = self.runtime.as_mut() else {
            self.status = "识别尚未启动".into();
            return;
        };
        let paused = !runtime.is_paused();
        runtime.set_paused(paused);
        self.last_output = Some(if paused {
            PipelineOutput::Paused
        } else {
            PipelineOutput::Waiting
        });
        self.status = if paused {
            "识别已暂停"
        } else {
            "识别已恢复，正在确认三帧"
        }
        .into();
    }

    fn refresh(&mut self) {
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.reset_recognition();
            self.manual_override = false;
            self.last_output = Some(PipelineOutput::Waiting);
            self.status = "重新识别中".into();
        } else {
            self.manual_tracker = GameTracker::new();
            self.last_output = None;
            self.status = "手动输入已清空".into();
        }
    }

    fn correct_hand(&mut self) {
        let mut buffer = [0u16; 256];
        let count = unsafe { GetWindowTextW(self.edit, buffer.as_mut_ptr(), buffer.len() as i32) };
        if count <= 0 {
            self.status = "输入空格分隔的牌码，例如 1m 2m 3m".into();
            return;
        }
        let input = String::from_utf16_lossy(&buffer[..count as usize]);
        let tiles: Result<Vec<Tile>, _> = input.split_whitespace().map(Tile::from_code).collect();
        let Ok(tiles) = tiles else {
            self.status = "牌码无效：万 m、筒 p、索 s、字 z，赤五用 0".into();
            return;
        };
        let outcome = if let Some(runtime) = self.runtime.as_mut() {
            runtime
                .tracker_mut()
                .correct_hand(tiles.clone(), 1.0)
                .map(|()| {
                    (
                        runtime.tracker().phase(),
                        runtime.tracker().discard_recommendations(),
                    )
                })
        } else {
            self.manual_tracker
                .correct_hand(tiles.clone(), 1.0)
                .map(|()| {
                    (
                        self.manual_tracker.phase(),
                        self.manual_tracker.discard_recommendations(),
                    )
                })
        };
        match outcome {
            Ok((phase, discards)) => {
                self.last_output = Some(PipelineOutput::Ready {
                    hand: tiles,
                    confidence: 1.0,
                    phase,
                    events: vec![],
                    discards,
                    available_actions: vec![],
                    actions: vec![],
                });
                self.manual_override = self.runtime.is_some();
                self.status = if self.manual_override {
                    "手牌修正已应用；点击“重识别”恢复自动识别"
                } else {
                    "手动牌效已更新；补齐本机模板后可使用自动识别"
                }
                .into();
            }
            Err(_) => self.status = "牌数或牌型不合法，请检查输入".into(),
        }
    }
}

fn paint(window: isize) {
    let mut paint = Paint::default();
    let dc = unsafe { BeginPaint(window, &mut paint) };
    if dc == 0 {
        return;
    }
    let mut area = Rect::default();
    unsafe {
        GetClientRect(window, &mut area);
        SelectObject(dc, GetStockObject(17));
        SetBkMode(dc, 1);
    }
    fill(dc, area, color(20, 27, 41));
    fill(
        dc,
        Rect {
            left: 0,
            top: 0,
            right: area.right,
            bottom: 36,
        },
        color(31, 47, 70),
    );
    draw_text(
        dc,
        12,
        10,
        "Riichi Screen Helper v0.0.1",
        color(235, 242, 255),
    );
    for (index, label) in [
        "找窗口",
        "校准",
        "开始",
        "暂停",
        "修正",
        "重识别",
        "隐藏",
        "退出",
    ]
    .iter()
    .enumerate()
    {
        let column = (index % 4) as i32;
        let row = (index / 4) as i32;
        let left = 12 + column * 113;
        let top = 48 + row * 40;
        fill(
            dc,
            Rect {
                left,
                top,
                right: left + 107,
                bottom: top + 34,
            },
            color(43, 63, 88),
        );
        draw_text(dc, left + 14, top + 8, label, color(231, 239, 248));
    }
    fill(
        dc,
        Rect {
            left: 370,
            top: 128,
            right: 458,
            bottom: 153,
        },
        color(43, 63, 88),
    );
    draw_text(dc, 383, 132, "学习模板", color(231, 239, 248));
    APP.with(|cell| {
        let borrow = cell.borrow();
        let Some(state) = borrow.as_ref() else {
            return;
        };
        draw_text(dc, 12, 156, &short(&state.status, 42), color(180, 214, 245));
        match state.last_output.as_ref() {
            Some(PipelineOutput::Paused) => {
                draw_text(dc, 12, 185, "已暂停；F8 恢复", color(243, 207, 123))
            }
            Some(PipelineOutput::Waiting) => {
                draw_text(dc, 12, 185, "等待三帧稳定画面…", color(243, 207, 123))
            }
            Some(PipelineOutput::Uncertain) => draw_text(
                dc,
                12,
                185,
                "识别不确定：建议已暂停，请确认牌面",
                color(255, 155, 145),
            ),
            Some(PipelineOutput::Ready {
                hand,
                confidence,
                phase,
                discards,
                available_actions,
                actions,
                ..
            }) => {
                let hand_text = hand
                    .iter()
                    .map(|tile| tile_code(*tile))
                    .collect::<Vec<_>>()
                    .join(" ");
                draw_text(
                    dc,
                    12,
                    181,
                    &format!("手牌：{}", short(&hand_text, 49)),
                    color(244, 244, 230),
                );
                draw_text(
                    dc,
                    12,
                    203,
                    &format!("状态：{:?}  识别置信度：{:.0}%", phase, confidence * 100.0),
                    color(182, 214, 234),
                );
                for (i, item) in discards.iter().take(3).enumerate() {
                    draw_text(
                        dc,
                        12,
                        225 + i as i32 * 23,
                        &format!(
                            "{}. 切 {}  {} 向听  {} 张有效牌",
                            item.rank,
                            tile_code(item.discard),
                            item.shanten,
                            item.ukeire
                        ),
                        if i == 0 {
                            color(139, 232, 182)
                        } else {
                            color(222, 231, 240)
                        },
                    );
                }
                if let Some(advice) = actions.first() {
                    draw_text(
                        dc,
                        12,
                        296,
                        &short(
                            &format!("行动：{}；{}", action_name(advice.action), advice.reason),
                            44,
                        ),
                        color(255, 217, 147),
                    );
                } else if !available_actions.is_empty() {
                    let names = available_actions
                        .iter()
                        .map(|action| action_name(*action))
                        .collect::<Vec<_>>()
                        .join("/");
                    draw_text(
                        dc,
                        12,
                        296,
                        &format!("按钮：{names}；来牌或役种未确认，暂不建议"),
                        color(255, 217, 147),
                    );
                }
            }
            None => draw_text(
                dc,
                12,
                185,
                "等待启动。手牌修正框使用 1m 2p 3s 4z 等牌码。",
                color(207, 215, 225),
            ),
        }
        draw_text(
            dc,
            12,
            area.bottom - 22,
            "F8 暂停   F9 显示/隐藏   F10 重识别",
            color(140, 158, 180),
        );
    });
    unsafe {
        EndPaint(window, &paint);
    }
}

fn fill(dc: isize, rect: Rect, color_value: u32) {
    let brush = unsafe { CreateSolidBrush(color_value) };
    if brush != 0 {
        unsafe {
            FillRect(dc, &rect, brush);
            DeleteObject(brush);
        }
    }
}

fn draw_text(dc: isize, x: i32, y: i32, value: &str, color_value: u32) {
    let encoded: Vec<u16> = value.encode_utf16().collect();
    unsafe {
        SetTextColor(dc, color_value);
        TextOutW(dc, x, y, encoded.as_ptr(), encoded.len() as i32);
    }
}

fn short(value: &str, max: usize) -> String {
    let mut text: String = value.chars().take(max).collect();
    if value.chars().count() > max {
        text.push('…');
    }
    text
}

fn tile_code(tile: Tile) -> String {
    let index = tile.index();
    let (number, suit) = if index < 9 {
        (index + 1, 'm')
    } else if index < 18 {
        (index - 8, 'p')
    } else if index < 27 {
        (index - 17, 's')
    } else {
        (index - 26, 'z')
    };
    format!("{}{}", if tile.is_red() { 0 } else { number }, suit)
}

fn action_name(action: vision_core::ActionLabel) -> &'static str {
    match action {
        vision_core::ActionLabel::None => "无",
        vision_core::ActionLabel::Chi => "吃",
        vision_core::ActionLabel::Pon => "碰",
        vision_core::ActionLabel::Kan => "杠",
        vision_core::ActionLabel::Riichi => "立直",
    }
}
