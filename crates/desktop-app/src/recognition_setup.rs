use runtime_core::RuntimeEngine;
use std::path::Path;
use vision_core::{
    MatchConfig, TemplateBank, TemplatePackProgress, VisionError,
    classifier::FaceClassifier,
    neural_classifier::{ClassifierLoadError, LocalTileClassifier},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecognitionMode {
    Templates,
    TemplatesAndModel,
    ModelAndActions,
}
impl RecognitionMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Templates => "模板模式",
            Self::TemplatesAndModel => "模板＋模型回退",
            Self::ModelAndActions => "模型识牌",
        }
    }
}
#[derive(Debug)]
pub(crate) enum SetupError {
    ManifestAccess,
    Model(ClassifierLoadError),
    Templates(TemplatePackProgress),
    Engine(VisionError),
}
impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestAccess => f.write_str("无法读取本机模型目录"),
            Self::Model(error) => f.write_str(match error {
                ClassifierLoadError::Manifest => "本机模型清单无法读取或格式不符",
                ClassifierLoadError::ModelFile => "本机模型文件缺失或大小不受支持",
                ClassifierLoadError::ModelHashMismatch => "本机模型与清单不一致，请检查文件",
                ClassifierLoadError::HashUnavailable => "系统模型校验服务不可用",
                ClassifierLoadError::Backend(_) => "Windows 无法加载此本机模型",
            }),
            Self::Templates(progress) => write!(
                f,
                "本机模板：牌图 {}/37、按钮 {}/5；可点击学习模板",
                progress.tiles, progress.actions
            ),
            Self::Engine(error) => write!(f, "识别启动失败：{error}"),
        }
    }
}

pub(crate) fn prepare_runtime(
    pack: &Path,
    model_manifest: &Path,
) -> Result<(RuntimeEngine, RecognitionMode), SetupError> {
    let classifier = if model_manifest
        .try_exists()
        .map_err(|_| SetupError::ManifestAccess)?
    {
        Some(LocalTileClassifier::load(model_manifest).map_err(SetupError::Model)?)
    } else {
        None
    };
    let (bank, mode) = match TemplateBank::load_dir(pack) {
        Ok(bank) => (
            bank,
            if classifier.is_some() {
                RecognitionMode::TemplatesAndModel
            } else {
                RecognitionMode::Templates
            },
        ),
        Err(_) if classifier.is_some() => (
            TemplateBank::load_actions_dir(pack)
                .map_err(|_| SetupError::Templates(TemplateBank::progress_dir(pack)))?,
            RecognitionMode::ModelAndActions,
        ),
        Err(_) => return Err(SetupError::Templates(TemplateBank::progress_dir(pack))),
    };
    let mut runtime =
        RuntimeEngine::new(bank, vec![], MatchConfig::default(), 3).map_err(SetupError::Engine)?;
    if let Some(classifier) = classifier {
        runtime.set_tile_classifier(Some(Box::new(classifier) as Box<dyn FaceClassifier>));
    }
    Ok((runtime, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use vision_core::{RgbImage, classifier::CLASS_ORDER};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p =
                std::env::temp_dir().join(format!("msh-startup-{}-{nonce}", std::process::id()));
            fs::create_dir(&p).unwrap();
            fs::create_dir(p.join("actions")).unwrap();
            fs::create_dir(p.join("tiles")).unwrap();
            Self(p)
        }
        fn buttons(&self) {
            for name in ["none", "chi", "pon", "kan", "riichi"] {
                RgbImage::new(18, 26, vec![240; 18 * 26 * 3])
                    .unwrap()
                    .write_png(&self.0.join("actions").join(format!("{name}.png")))
                    .unwrap();
            }
        }
        fn all_tiles(&self) {
            for name in CLASS_ORDER {
                RgbImage::new(18, 26, vec![240; 18 * 26 * 3])
                    .unwrap()
                    .write_png(&self.0.join("tiles").join(format!("{name}.png")))
                    .unwrap();
            }
        }
        fn no_model(&self) -> PathBuf {
            self.0.join("missing-model.json")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for folder in ["actions", "tiles"] {
                if let Ok(files) = fs::read_dir(self.0.join(folder)) {
                    for p in files.flatten() {
                        let _ = fs::remove_file(p.path());
                    }
                }
                let _ = fs::remove_dir(self.0.join(folder));
            }
            let _ = fs::remove_file(self.0.join("bad-model.json"));
            let _ = fs::remove_dir(&self.0);
        }
    }
    fn model_manifest() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/synthetic/neural/classifier.json")
    }

    #[test]
    fn models_allow_actions_only_but_do_not_allow_missing_action_states() {
        let f = Fixture::new();
        f.buttons();
        let (_, mode) = prepare_runtime(&f.0, &model_manifest()).unwrap();
        assert_eq!(mode, RecognitionMode::ModelAndActions);
        fs::remove_file(f.0.join("actions/kan.png")).unwrap();
        assert!(matches!(
            prepare_runtime(&f.0, &model_manifest()),
            Err(SetupError::Templates(_))
        ));
    }
    #[test]
    fn complete_templates_work_without_models_and_optionally_keep_a_model_fallback() {
        let f = Fixture::new();
        f.buttons();
        f.all_tiles();
        let (_, mode) = prepare_runtime(&f.0, &f.no_model()).unwrap();
        assert_eq!(mode, RecognitionMode::Templates);
        let (_, mode) = prepare_runtime(&f.0, &model_manifest()).unwrap();
        assert_eq!(mode, RecognitionMode::TemplatesAndModel);
    }
    #[test]
    fn an_installed_invalid_model_never_silently_downgrades_to_templates() {
        let f = Fixture::new();
        f.buttons();
        f.all_tiles();
        let bad = f.0.join("bad-model.json");
        fs::write(&bad, b"{}").unwrap();
        assert!(matches!(
            prepare_runtime(&f.0, &bad),
            Err(SetupError::Model(_))
        ));
        assert!(prepare_runtime(&f.0, &f.no_model()).is_ok());
    }
}
