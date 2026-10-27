//! Scoped WinRT activation factories. The projection's static factory caches
//! outlive a per-session apartment; reusing them after its last MTA is torn down
//! caused native access violations in sequential load/error/load sequences.
//! Query factories through the public uncached `windows::core::factory` API and
//! release them before the caller's apartment guard. No MTA cookie is leaked.

use std::{ffi::c_void, ptr::null_mut};
use windows::{
    AI::MachineLearning::{
        ILearningModelBindingFactory, ILearningModelDeviceFactory, ILearningModelSessionFactory,
        ILearningModelStatics, ITensorFloatStatics2, LearningModel, LearningModelBinding,
        LearningModelDevice, LearningModelDeviceKind, LearningModelSession, TensorFloat,
    },
    core::{HRESULT, HSTRING, Interface, Result, factory},
};

// Safety: each caller supplies an ABI out-pointer for the requested class and
// checks its HRESULT before transferring the returned owned COM reference.
unsafe fn created<T: Interface>(code: HRESULT, raw: *mut c_void) -> Result<T> {
    code.ok()?;
    if raw.is_null() {
        return Err(windows::core::Error::from_hresult(HRESULT(
            0x80004003u32 as i32,
        )));
    }
    Ok(unsafe { T::from_raw(raw) })
}

pub(super) fn load_model(path: &HSTRING) -> Result<LearningModel> {
    let factory = factory::<LearningModel, ILearningModelStatics>()?;
    let mut result = null_mut();
    // HSTRING's borrowed ABI representation matches the generated projection;
    // `path` and the factory remain alive for the entire synchronous call.
    unsafe {
        let code = (factory.vtable().LoadFromFilePath)(
            factory.as_raw(),
            std::mem::transmute_copy(path),
            &mut result,
        );
        created(code, result)
    }
}

pub(super) fn cpu_session(model: &LearningModel) -> Result<LearningModelSession> {
    let device_factory = factory::<LearningModelDevice, ILearningModelDeviceFactory>()?;
    let device: LearningModelDevice = unsafe {
        let mut result = null_mut();
        let code = (device_factory.vtable().Create)(
            device_factory.as_raw(),
            LearningModelDeviceKind::Cpu,
            &mut result,
        );
        created(code, result)?
    };
    let session_factory = factory::<LearningModelSession, ILearningModelSessionFactory>()?;
    unsafe {
        let mut result = null_mut();
        let code = (session_factory.vtable().CreateFromModelOnDevice)(
            session_factory.as_raw(),
            model.as_raw(),
            device.as_raw(),
            &mut result,
        );
        created(code, result)
    }
}

pub(super) fn input_tensor(input: &[f32]) -> Result<TensorFloat> {
    let factory = factory::<TensorFloat, ITensorFloatStatics2>()?;
    let shape = [1i64, 3, 48, 32];
    unsafe {
        let mut result = null_mut();
        let code = (factory.vtable().CreateFromShapeArrayAndDataArray)(
            factory.as_raw(),
            shape.len() as u32,
            shape.as_ptr(),
            input.len() as u32,
            input.as_ptr(),
            &mut result,
        );
        created(code, result)
    }
}

pub(super) fn binding(session: &LearningModelSession) -> Result<LearningModelBinding> {
    let factory = factory::<LearningModelBinding, ILearningModelBindingFactory>()?;
    unsafe {
        let mut result = null_mut();
        let code =
            (factory.vtable().CreateFromSession)(factory.as_raw(), session.as_raw(), &mut result);
        created(code, result)
    }
}
