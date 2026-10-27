use screen_capture::{CaptureError, NormalizedRect, PixelRect, RegionKind};

#[cfg(windows)]
#[test]
fn captures_a_real_window_client_region_without_saving_a_file() {
    use screen_capture::{
        CaptureBackend, CaptureStream, GraphicsSession, calibrate_window, capture_region,
        find_windows, select_region,
    };
    use std::ffi::c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
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
        fn ShowWindow(window: isize, command: i32) -> i32;
        fn UpdateWindow(window: isize) -> i32;
        fn DestroyWindow(window: isize) -> i32;
        fn MoveWindow(window: isize, x: i32, y: i32, width: i32, height: i32, repaint: i32) -> i32;
        fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let title = format!("Mahjong Capture Fixture {}", std::process::id());
    let name: Vec<u16> = format!("{title}\0").encode_utf16().collect();
    const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            name.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            80,
            80,
            320,
            240,
            0,
            0,
            0,
            std::ptr::null(),
        )
    };
    assert_ne!(hwnd, 0, "CreateWindowExW failed");
    struct WindowGuard(isize);
    impl Drop for WindowGuard {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    let _guard = WindowGuard(hwnd);
    unsafe {
        ShowWindow(hwnd, 5);
        UpdateWindow(hwnd);
    }

    let found = find_windows(&title).unwrap();
    let window = found
        .iter()
        .find(|window| window.handle.0 == hwnd)
        .expect("fixture window not found");
    assert!(window.client.width >= 200 && window.client.height >= 100);
    let region = NormalizedRect::new(0.1, 0.1, 0.9, 0.9).unwrap();
    let frame = capture_region(window.handle, region).unwrap();
    assert_eq!(
        frame.bgra.len(),
        frame.size.width as usize * frame.size.height as usize * 4
    );
    assert!(frame.dpi >= 96);

    assert_ne!(unsafe { MoveWindow(hwnd, 160, 120, 480, 360, 1) }, 0);
    let resized = find_windows(&title).unwrap();
    let new_window = resized
        .iter()
        .find(|candidate| candidate.handle.0 == hwnd)
        .unwrap();
    assert!(new_window.client.width > window.client.width);
    let new_frame = capture_region(new_window.handle, region).unwrap();
    assert!(new_frame.size.width > frame.size.width);
    assert_eq!(new_frame.region, region.resolve(new_window.client).unwrap());

    let graphics = GraphicsSession::start(new_window.handle, region).unwrap();
    let graphics_frame = graphics
        .next_frame(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(graphics_frame.region, new_frame.region);
    assert_eq!(graphics_frame.size, new_frame.size);
    assert_eq!(graphics_frame.bgra.len(), new_frame.bgra.len());
    assert_eq!(graphics_frame.client, new_window.client);
    let crop_region = NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap();
    let same_frame_crop = graphics_frame.crop_normalized(crop_region).unwrap();
    assert_eq!(
        same_frame_crop.region,
        crop_region.resolve(new_window.client).unwrap()
    );
    assert_eq!(same_frame_crop.source, graphics_frame.source);

    assert_ne!(unsafe { MoveWindow(hwnd, 200, 140, 600, 400, 1) }, 0);
    let latest = find_windows(&title).unwrap();
    let latest_window = latest
        .iter()
        .find(|candidate| candidate.handle.0 == hwnd)
        .unwrap();
    let expected = region
        .resolve(latest_window.client)
        .unwrap()
        .size()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut updated = false;
    while std::time::Instant::now() < deadline {
        if let Ok(frame) = graphics.next_frame(std::time::Duration::from_millis(500))
            && frame.size == expected
        {
            updated = true;
            break;
        }
    }
    assert!(
        updated,
        "graphics capture did not adapt to resized client area"
    );
    graphics.stop().unwrap();

    let mut stream = CaptureStream::start(latest_window.handle, region).unwrap();
    assert_eq!(stream.backend(), CaptureBackend::GraphicsCapture);
    let streamed = stream
        .next_frame(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(streamed.size, expected);
    let idle = stream.next_frame(std::time::Duration::from_millis(50));
    assert!(idle.is_ok() || idle == Err(CaptureError::Timeout));
    assert_eq!(stream.backend(), CaptureBackend::GraphicsCapture);

    let selection = std::thread::spawn(move || {
        select_region(screen_capture::WindowHandle(hwnd), "automated fixture")
    });
    let overlay = (0..200)
        .find_map(|_| {
            let windows =
                find_windows("Mahjong Screen Helper Calibration: automated fixture").unwrap();
            if let Some(window) = windows.first() {
                Some(window.handle.0)
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                None
            }
        })
        .expect("calibration overlay did not open");
    let point = |x: u16, y: u16| (x as isize) | ((y as isize) << 16);
    assert_ne!(
        unsafe { PostMessageW(overlay, 0x0201, 0, point(20, 20)) },
        0
    );
    assert_ne!(
        unsafe { PostMessageW(overlay, 0x0200, 0, point(120, 90)) },
        0
    );
    assert_ne!(
        unsafe { PostMessageW(overlay, 0x0202, 0, point(120, 90)) },
        0
    );
    let chosen = selection.join().unwrap().unwrap();
    assert_eq!(
        chosen.resolve(latest_window.client).unwrap(),
        PixelRect {
            left: 20,
            top: 20,
            right: 120,
            bottom: 90
        }
    );

    let cancelled = std::thread::spawn(move || {
        select_region(screen_capture::WindowHandle(hwnd), "cancel fixture")
    });
    let cancel_overlay = (0..200)
        .find_map(|_| {
            let windows =
                find_windows("Mahjong Screen Helper Calibration: cancel fixture").unwrap();
            if let Some(window) = windows.first() {
                Some(window.handle.0)
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                None
            }
        })
        .expect("cancel overlay did not open");
    assert_ne!(unsafe { PostMessageW(cancel_overlay, 0x0100, 27, 0) }, 0);
    assert_eq!(cancelled.join().unwrap(), Err(CaptureError::Cancelled));

    let calibration_window = latest_window.clone();
    let calibration = std::thread::spawn(move || {
        calibrate_window(&calibration_window, "Mahjong Capture Fixture")
    });
    for fragment in [
        "自己的手牌区域",
        "自己的副露区域",
        "可见牌河区域",
        "宝牌指示牌区域",
        "行动按钮区域",
        "上家（左侧）副露区域",
        "对家（上方）副露区域",
        "下家（右侧）副露区域",
    ] {
        let selector = (0..200)
            .find_map(|_| {
                let windows = find_windows(fragment).unwrap();
                if let Some(window) = windows.first() {
                    Some(window.handle.0)
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    None
                }
            })
            .expect("calibration step did not open");
        assert_ne!(
            unsafe { PostMessageW(selector, 0x0201, 0, point(10, 10)) },
            0
        );
        assert_ne!(
            unsafe { PostMessageW(selector, 0x0202, 0, point(50, 50)) },
            0
        );
    }
    let profile = calibration.join().unwrap().unwrap();
    assert!(profile.is_complete());
    for kind in RegionKind::ALL {
        assert_eq!(
            profile
                .region(kind)
                .unwrap()
                .resolve(latest_window.client)
                .unwrap(),
            PixelRect {
                left: 10,
                top: 10,
                right: 50,
                bottom: 50
            }
        );
    }
}
