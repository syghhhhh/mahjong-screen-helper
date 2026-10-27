use screen_capture::{CalibrationProfile, NormalizedRect, PixelRect, RegionKind, Size};

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
