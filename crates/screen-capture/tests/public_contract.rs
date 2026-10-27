use screen_capture::{
    CalibrationProfile, CaptureError, NormalizedRect, PixelRect, RegionKind, Size,
};

#[test]
fn calibrated_region_scales_with_window_and_dpi() {
    let calibration = NormalizedRect::from_pixels(
        Size {
            width: 1920,
            height: 1080,
        },
        PixelRect {
            left: 192,
            top: 756,
            right: 1728,
            bottom: 972,
        },
    )
    .unwrap();
    assert_eq!(
        calibration
            .resolve(Size {
                width: 1280,
                height: 720
            })
            .unwrap(),
        PixelRect {
            left: 128,
            top: 504,
            right: 1152,
            bottom: 648
        }
    );
    assert_eq!(
        calibration
            .resolve(Size {
                width: 2400,
                height: 1350
            })
            .unwrap(),
        PixelRect {
            left: 240,
            top: 945,
            right: 2160,
            bottom: 1215
        }
    );
    assert_eq!(
        calibration
            .resolve(Size {
                width: 2880,
                height: 1620
            })
            .unwrap(),
        PixelRect {
            left: 288,
            top: 1134,
            right: 2592,
            bottom: 1458
        }
    );
}

#[test]
fn region_must_stay_inside_client_area() {
    assert!(
        NormalizedRect::from_pixels(
            Size {
                width: 100,
                height: 100
            },
            PixelRect {
                left: -1,
                top: 10,
                right: 50,
                bottom: 50
            },
        )
        .is_err()
    );
    assert!(
        NormalizedRect::from_pixels(
            Size {
                width: 100,
                height: 100
            },
            PixelRect {
                left: 90,
                top: 10,
                right: 101,
                bottom: 50
            },
        )
        .is_err()
    );
    assert!(NormalizedRect::new(0.5, 0.5, 0.5, 0.9).is_err());
    assert!(NormalizedRect::new(f32::NAN, 0.0, 1.0, 1.0).is_err());
}

#[test]
fn calibration_profile_round_trips_without_image_data_and_rejects_wrong_schema() {
    let client = Size {
        width: 1920,
        height: 1080,
    };
    let region = NormalizedRect::from_pixels(
        client,
        PixelRect {
            left: 100,
            top: 100,
            right: 500,
            bottom: 300,
        },
    )
    .unwrap();
    let mut profile = CalibrationProfile::new("雀魂", client, 120).unwrap();
    for kind in RegionKind::ALL {
        profile.set_region(kind, region).unwrap();
    }
    let path = std::env::temp_dir().join(format!("msh-calibration-{}.json", std::process::id()));
    profile.save(&path).unwrap();
    let restored = CalibrationProfile::load(&path).unwrap();
    assert_eq!(restored, profile);
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("bgra") && !content.contains("screenshot"));
    std::fs::write(
        &path,
        content.replace("\"schema_version\": 1", "\"schema_version\": 2"),
    )
    .unwrap();
    assert!(CalibrationProfile::load(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

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
