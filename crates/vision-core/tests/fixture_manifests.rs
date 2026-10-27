//! Validate committed annotation semantics without requiring private screenshots.
use riichi_core::Tile;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/manifests")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn check_honor(code: &str, observed: Option<&str>) -> Result<(), String> {
    let tile = Tile::from_code(code).map_err(|e| e.to_string())?;
    if tile.index() < 27 {
        return if observed.is_none() {
            Ok(())
        } else {
            Err(format!("non-honor {code} has an honor observation"))
        };
    }
    let observed = observed.ok_or_else(|| format!("missing honor observation for {code}"))?;
    let expected = match observed {
        "east" => "1z",
        "south" => "2z",
        "west" => "3z",
        "north" => "4z",
        "white" => "5z",
        "green" => "6z",
        "red" => "7z",
        _ => return Err(format!("unknown observed honor {observed}")),
    };
    if code != expected {
        return Err(format!(
            "observed {observed} requires {expected}, got {code}"
        ));
    }
    Ok(())
}

fn check_hand(row: &Value) -> Result<(), String> {
    let hand: Vec<_> = row["expected_hand"]
        .as_str()
        .ok_or("missing hand")?
        .split_whitespace()
        .collect();
    let annotations = row["observed_honors"]
        .as_array()
        .ok_or("missing observed_honors array")?;
    let mut by_position = BTreeMap::new();
    for annotation in annotations {
        let position = annotation["one_based_tile_position"]
            .as_u64()
            .ok_or("invalid annotation position")?;
        if position == 0 || position > hand.len() as u64 {
            return Err("out-of-range annotation position".into());
        }
        let observed = annotation["observed_honor"]
            .as_str()
            .ok_or("missing observed honor name")?;
        if by_position.insert(position as usize, observed).is_some() {
            return Err("duplicate honor annotation".into());
        }
    }
    let mut counts = [0; 34];
    for (index, code) in hand.iter().enumerate() {
        check_honor(code, by_position.get(&(index + 1)).copied())?;
        let tile = Tile::from_code(code).map_err(|e| e.to_string())?;
        counts[tile.index() as usize] += 1;
    }
    if counts.iter().any(|&n| n > 4) {
        return Err("more than four physical copies in annotated hand".into());
    }
    Ok(())
}

fn check_manifest(document: &Value) -> Result<(), String> {
    let mut checked = false;
    if let Some(cases) = document["cases"].as_array() {
        for row in cases {
            check_hand(row)?;
            checked = true;
        }
    }
    if document.get("expected_hand").is_some() {
        check_hand(document)?;
        checked = true;
    }
    for key in ["faces", "public_faces"] {
        if let Some(rows) = document[key].as_array() {
            for row in rows {
                let code = row["tile"].as_str().ok_or("missing public tile code")?;
                let observed = match row.get("observed_honor") {
                    None => None,
                    Some(v) => Some(v.as_str().ok_or("observed honor must be text")?),
                };
                check_honor(code, observed)?;
                checked = true;
            }
        }
    }
    if !checked {
        return Err("unrecognized or empty visual truth manifest".into());
    }
    Ok(())
}

#[test]
fn committed_visual_truth_uses_consistent_observed_honor_names_and_codes() {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/manifests");
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|x| x == "json") {
            let doc: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert!(
                check_manifest(&doc).is_ok(),
                "{}: {:?}",
                path.display(),
                check_manifest(&doc)
            );
        }
    }
}

#[test]
fn west_as_north_mistranscriptions_fail_for_public_and_hand_truth() {
    let mut board = fixture("public-board-v1.json");
    let face = board["faces"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["observed_honor"] == "west")
        .unwrap();
    face["tile"] = "4z".into();
    let error = check_manifest(&board).unwrap_err();
    assert!(error.contains("west") && error.contains("3z") && error.contains("4z"));
    let mut scene = fixture("public-scene-v1.json");
    let mut hand: Vec<_> = scene["expected_hand"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    hand[11] = "4z".into();
    scene["expected_hand"] = hand.join(" ").into();
    let error = check_manifest(&scene).unwrap_err();
    assert!(error.contains("west") && error.contains("3z") && error.contains("4z"));
}

#[test]
fn missing_typo_and_duplicate_honor_observations_cannot_satisfy_annotation_checks() {
    let scene = fixture("public-scene-v1.json");
    let mut missing = scene.clone();
    missing["observed_honors"].as_array_mut().unwrap().clear();
    assert!(
        check_manifest(&missing)
            .unwrap_err()
            .contains("missing honor observation")
    );
    let mut typo = scene.clone();
    typo["observed_honors"][0]["observed_honor"] = "wset".into();
    assert!(
        check_manifest(&typo)
            .unwrap_err()
            .contains("unknown observed honor wset")
    );
    let mut duplicate = scene;
    let annotation = duplicate["observed_honors"][0].clone();
    duplicate["observed_honors"]
        .as_array_mut()
        .unwrap()
        .push(annotation);
    assert!(
        check_manifest(&duplicate)
            .unwrap_err()
            .contains("duplicate honor annotation")
    );
}
