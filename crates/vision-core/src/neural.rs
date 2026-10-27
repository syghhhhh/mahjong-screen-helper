//! Optional, local ONNX execution through the Windows 10/11 system runtime.
//! This is a bounded tensor backend, not a validated Mahjong recognizer. Its
//! caller must validate the model's class map, preprocessing, and confidence.

use std::{io::Read, marker::PhantomData, os::windows::ffi::OsStrExt, path::Path, rc::Rc};
use windows::{
    AI::MachineLearning::{
        ILearningModelFeatureDescriptor, LearningModelFeatureKind, LearningModelSession,
        TensorFeatureDescriptor, TensorFloat, TensorKind,
    },
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{HSTRING, Interface},
};

mod activation;

pub use crate::neural_input::INPUT_FLOATS;
pub const OUTPUT_CLASSES: usize = 37;
pub const ACTION_INPUT_FLOATS: usize = crate::action_input::INPUT_FLOATS;
pub const ACTION_OUTPUT_CLASSES: usize = 7;
const MAX_MODEL_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeuralError {
    InvalidModel,
    UnsupportedModel,
    BackendUnavailable,
    InvalidInput,
    InvalidOutput,
    InferenceFailed,
}

impl std::fmt::Display for NeuralError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for NeuralError {}

// Apartment lifetime and session destruction stay on the creating thread.
// Agile WinRT interfaces must not accidentally move this RoUninitialize call.
struct Apartment {
    owned: bool,
    _thread: PhantomData<Rc<()>>,
}

impl Apartment {
    fn initialize() -> Result<Self, NeuralError> {
        let owned = match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => true,
            // A caller-owned STA can use this synchronous WinRT API too.
            Err(error) if error.code().0 as u32 == 0x80010106 => false,
            Err(_) => return Err(NeuralError::BackendUnavailable),
        };
        Ok(Self {
            owned,
            _thread: PhantomData,
        })
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.owned {
            unsafe { RoUninitialize() };
        }
    }
}

/// CPU-only ONNX session with one float32 NCHW [1,3,48,32] input and one
/// float32 [1,37] logits output. No Python or third-party inference DLL is used.
pub struct WinMlTileSession(TensorSession<OUTPUT_CLASSES>);

impl WinMlTileSession {
    pub fn load(path: &Path) -> Result<Self, NeuralError> {
        TensorSession::load(path, [1, 3, 48, 32]).map(Self)
    }
    pub fn evaluate(&self, input: &[f32]) -> Result<[f32; OUTPUT_CLASSES], NeuralError> {
        self.0.evaluate(input)
    }
}

/// Separate action-word tensor contract. This returns raw logits only; it
/// neither locates buttons nor validates their labels, context or confidence.
pub struct WinMlActionSession(TensorSession<ACTION_OUTPUT_CLASSES>);

impl WinMlActionSession {
    pub fn load(path: &Path) -> Result<Self, NeuralError> {
        TensorSession::load(path, [1, 1, 64, 128]).map(Self)
    }
    pub fn evaluate(&self, input: &[f32]) -> Result<[f32; ACTION_OUTPUT_CLASSES], NeuralError> {
        self.0.evaluate(input)
    }
}

// Only the two public, fixed contracts above can create this backend. No
// manifest or caller can supply an arbitrary tensor shape or output count.
struct TensorSession<const CLASSES: usize> {
    session: LearningModelSession,
    input_name: HSTRING,
    output_name: HSTRING,
    input_shape: [i64; 4],
    // Rust drops fields in declaration order, so COM objects go first.
    _apartment: Apartment,
}

impl<const CLASSES: usize> TensorSession<CLASSES> {
    fn load(path: &Path, input_shape: [i64; 4]) -> Result<Self, NeuralError> {
        // Reject obvious non-model files before entering the system parser.
        // This is a format sanity check, not a full ONNX protobuf validator.
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("onnx"))
        {
            return Err(NeuralError::InvalidModel);
        }
        let metadata = std::fs::metadata(path).map_err(|_| NeuralError::InvalidModel)?;
        if !metadata.is_file() || metadata.len() < 16 || metadata.len() > MAX_MODEL_BYTES {
            return Err(NeuralError::InvalidModel);
        }
        let mut prefix = [0; 2];
        std::fs::File::open(path)
            .and_then(|mut file| file.read_exact(&mut prefix))
            .map_err(|_| NeuralError::InvalidModel)?;
        // Our supported exporter writes canonical ir_version as its first field.
        if prefix[0] != 0x08 || !(3..=10).contains(&prefix[1]) {
            return Err(NeuralError::InvalidModel);
        }
        let absolute = std::path::absolute(path).map_err(|_| NeuralError::InvalidModel)?;
        let path = HSTRING::from_wide(&absolute.as_os_str().encode_wide().collect::<Vec<_>>());
        let apartment = Apartment::initialize()?;
        let model = activation::load_model(&path).map_err(|error| {
            // Preserve the system rejection in development/CI diagnostics.
            // The public error remains conservative; an intermittent load
            // failure must not be reclassified as an accepted model shape.
            #[cfg(debug_assertions)]
            eprintln!(
                "WinML LoadFromFilePath failed: HRESULT=0x{:08X}; {}",
                error.code().0 as u32,
                error.message()
            );
            #[cfg(not(debug_assertions))]
            let _ = error;
            NeuralError::InvalidModel
        })?;
        let inputs = model
            .InputFeatures()
            .map_err(|_| NeuralError::UnsupportedModel)?;
        let outputs = model
            .OutputFeatures()
            .map_err(|_| NeuralError::UnsupportedModel)?;
        if inputs.Size().map_err(|_| NeuralError::UnsupportedModel)? != 1
            || outputs.Size().map_err(|_| NeuralError::UnsupportedModel)? != 1
        {
            return Err(NeuralError::UnsupportedModel);
        }
        let input = inputs.GetAt(0).map_err(|_| NeuralError::UnsupportedModel)?;
        let output = outputs
            .GetAt(0)
            .map_err(|_| NeuralError::UnsupportedModel)?;
        validate_feature(&input, &input_shape)?;
        validate_feature(&output, &[1, CLASSES as i64])?;
        let input_name = input.Name().map_err(|_| NeuralError::UnsupportedModel)?;
        let output_name = output.Name().map_err(|_| NeuralError::UnsupportedModel)?;
        let session =
            activation::cpu_session(&model).map_err(|_| NeuralError::BackendUnavailable)?;
        Ok(Self {
            session,
            input_name,
            output_name,
            input_shape,
            _apartment: apartment,
        })
    }

    /// Return finite raw logits. A successful inference says nothing about a
    /// model's real-game accuracy and never changes a task's acceptance state.
    fn evaluate(&self, input: &[f32]) -> Result<[f32; CLASSES], NeuralError> {
        if input.len()
            != self
                .input_shape
                .iter()
                .map(|&d| d as usize)
                .product::<usize>()
            || input
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(NeuralError::InvalidInput);
        }
        let tensor = activation::input_tensor(input, &self.input_shape)
            .map_err(|_| NeuralError::InferenceFailed)?;
        let binding =
            activation::binding(&self.session).map_err(|_| NeuralError::InferenceFailed)?;
        binding
            .Bind(&self.input_name, &tensor)
            .map_err(|_| NeuralError::InferenceFailed)?;
        let result = self
            .session
            .Evaluate(&binding, &HSTRING::new())
            .map_err(|_| NeuralError::InferenceFailed)?;
        let outputs = result.Outputs().map_err(|_| NeuralError::InvalidOutput)?;
        let tensor: TensorFloat = outputs
            .Lookup(&self.output_name)
            .and_then(|value| value.cast())
            .map_err(|_| NeuralError::InvalidOutput)?;
        let shape = tensor.Shape().map_err(|_| NeuralError::InvalidOutput)?;
        if shape.Size().map_err(|_| NeuralError::InvalidOutput)? != 2
            || shape.GetAt(0).map_err(|_| NeuralError::InvalidOutput)? != 1
            || shape.GetAt(1).map_err(|_| NeuralError::InvalidOutput)? != CLASSES as i64
        {
            return Err(NeuralError::InvalidOutput);
        }
        let values = tensor
            .GetAsVectorView()
            .map_err(|_| NeuralError::InvalidOutput)?;
        if values.Size().map_err(|_| NeuralError::InvalidOutput)? != CLASSES as u32 {
            return Err(NeuralError::InvalidOutput);
        }
        let mut logits = [0.0; CLASSES];
        for (index, value) in logits.iter_mut().enumerate() {
            *value = values
                .GetAt(index as u32)
                .map_err(|_| NeuralError::InvalidOutput)?;
            if !value.is_finite() {
                return Err(NeuralError::InvalidOutput);
            }
        }
        Ok(logits)
    }
}

impl<const CLASSES: usize> Drop for TensorSession<CLASSES> {
    fn drop(&mut self) {
        let _ = self.session.Close();
    }
}

fn validate_feature(
    feature: &ILearningModelFeatureDescriptor,
    expected: &[i64],
) -> Result<(), NeuralError> {
    if feature.Kind().map_err(|_| NeuralError::UnsupportedModel)?
        != LearningModelFeatureKind::Tensor
    {
        return Err(NeuralError::UnsupportedModel);
    }
    let tensor: TensorFeatureDescriptor =
        feature.cast().map_err(|_| NeuralError::UnsupportedModel)?;
    if tensor
        .TensorKind()
        .map_err(|_| NeuralError::UnsupportedModel)?
        != TensorKind::Float
    {
        return Err(NeuralError::UnsupportedModel);
    }
    let shape = tensor.Shape().map_err(|_| NeuralError::UnsupportedModel)?;
    if shape.Size().map_err(|_| NeuralError::UnsupportedModel)? != expected.len() as u32 {
        return Err(NeuralError::UnsupportedModel);
    }
    for (index, dimension) in expected.iter().enumerate() {
        if shape
            .GetAt(index as u32)
            .map_err(|_| NeuralError::UnsupportedModel)?
            != *dimension
        {
            return Err(NeuralError::UnsupportedModel);
        }
    }
    Ok(())
}
