//! Explicit local model loading; no downloads or automatic overlay activation.
use crate::{
    classifier::{
        ClassifierDecision, ClassifierFailure, FaceClassifier, FaceObservation, ModelManifest,
    },
    neural::{NeuralError, WinMlTileSession},
};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::fs::OpenOptionsExt,
    path::Path,
};
use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassifierLoadError {
    Manifest,
    ModelFile,
    ModelHashMismatch,
    HashUnavailable,
    Backend(NeuralError),
}

impl FaceClassifier for LocalTileClassifier {
    fn classify(&self, face: &FaceObservation) -> Result<ClassifierDecision, ClassifierFailure> {
        LocalTileClassifier::classify(self, face).map_err(|error| match error {
            NeuralError::InvalidInput => ClassifierFailure::InvalidInput,
            NeuralError::InvalidOutput => ClassifierFailure::InvalidOutput,
            _ => ClassifierFailure::Unavailable,
        })
    }
}
impl std::fmt::Display for ClassifierLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ClassifierLoadError {}

/// A hash-bound classifier and manifest. The model remains read-locked until
/// after the native session is destroyed. The creating thread owns its COM life.
pub struct LocalTileClassifier {
    session: WinMlTileSession,
    manifest: ModelManifest,
    _model_file: File,
}
impl LocalTileClassifier {
    pub fn load(manifest_path: &Path) -> Result<Self, ClassifierLoadError> {
        let mut bytes = Vec::new();
        File::open(manifest_path)
            .and_then(|file| file.take(16385).read_to_end(&mut bytes))
            .map_err(|_| ClassifierLoadError::Manifest)?;
        let manifest = ModelManifest::parse(&bytes).map_err(|_| ClassifierLoadError::Manifest)?;
        let model_path = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(manifest.model_file());
        // FILE_SHARE_READ allows the system loader to read, but prevents writes
        // or replacement between hashing, loading, and subsequent inference.
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&model_path)
            .map_err(|_| ClassifierLoadError::ModelFile)?;
        let size = file
            .metadata()
            .map_err(|_| ClassifierLoadError::ModelFile)?
            .len();
        if !(16..=16 * 1024 * 1024).contains(&size) {
            return Err(ClassifierLoadError::ModelFile);
        }
        bytes.clear();
        (&mut file)
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ClassifierLoadError::ModelFile)?;
        if bytes.len() as u64 != size {
            return Err(ClassifierLoadError::ModelFile);
        }
        if sha256_hex(&bytes)? != manifest.model_sha256() {
            return Err(ClassifierLoadError::ModelHashMismatch);
        }
        let session = WinMlTileSession::load(&model_path).map_err(ClassifierLoadError::Backend)?;
        Ok(Self {
            session,
            manifest,
            _model_file: file,
        })
    }
    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }
    pub fn classify(&self, face: &FaceObservation) -> Result<ClassifierDecision, NeuralError> {
        let logits = self.session.evaluate(face.input())?;
        Ok(self.manifest.decide(face, &logits))
    }
}

fn sha256_hex(bytes: &[u8]) -> Result<String, ClassifierLoadError> {
    let mut hash = [0u8; 32];
    // CNG's Windows 10 pseudo-handle has no owned lifetime or COM factory cache.
    let status = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut hash) };
    if status.0 < 0 {
        return Err(ClassifierLoadError::HashUnavailable);
    }
    Ok(hash.iter().map(|v| format!("{v:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_hash_matches_independent_sha256_vectors() {
        assert_eq!(
            sha256_hex(b"").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
