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
    retry_stopping_server(|| load_model_once(path), std::thread::sleep)
}

// A just-stopped COM object server can reject a new activation on Windows
// Server. Retry only that explicit HRESULT, never format/shape/inference errors.
// Keep every attempt inside the caller's live apartment, with no cached factory.
fn retry_stopping_server<T>(
    mut operation: impl FnMut() -> Result<T>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<T> {
    let mut delays = [10, 50, 100].into_iter();
    loop {
        match operation() {
            Err(error) if error.code().0 as u32 == 0x80080008 => {
                let Some(delay) = delays.next() else {
                    return Err(error);
                };
                #[cfg(debug_assertions)]
                eprintln!("WinML activation: CO_E_SERVER_STOPPING; bounded retry after {delay} ms");
                wait(std::time::Duration::from_millis(delay));
            }
            result => return result,
        }
    }
}

fn load_model_once(path: &HSTRING) -> Result<LearningModel> {
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

pub(super) fn input_tensor(input: &[f32], shape: &[i64; 4]) -> Result<TensorFloat> {
    let factory = factory::<TensorFloat, ITensorFloatStatics2>()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};

    #[test]
    fn stopping_server_recovery_is_bounded_and_preserves_the_actual_result() {
        let calls = Cell::new(0);
        let waits = std::cell::RefCell::new(Vec::new());
        let result = retry_stopping_server(
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 3 {
                    Err(windows::core::Error::from_hresult(HRESULT(
                        0x80080008u32 as i32,
                    )))
                } else {
                    Ok(17)
                }
            },
            |duration| waits.borrow_mut().push(duration),
        );
        assert_eq!(result.unwrap(), 17);
        assert_eq!(calls.get(), 3);
        assert_eq!(
            *waits.borrow(),
            [Duration::from_millis(10), Duration::from_millis(50)]
        );
    }

    #[test]
    fn persistent_shutdown_and_other_errors_remain_failures() {
        for (code, expected_calls, expected_waits) in [(0x80080008u32, 4, 3), (0x80070057, 1, 0)] {
            let calls = Cell::new(0);
            let waits = Cell::new(0);
            let result: Result<()> = retry_stopping_server(
                || {
                    calls.set(calls.get() + 1);
                    Err(windows::core::Error::from_hresult(HRESULT(code as i32)))
                },
                |_| waits.set(waits.get() + 1),
            );
            assert_eq!(result.unwrap_err().code().0 as u32, code);
            assert_eq!(calls.get(), expected_calls);
            assert_eq!(waits.get(), expected_waits);
        }
    }

    #[test]
    fn immediate_success_never_sleeps_or_repeats() {
        let calls = Cell::new(0);
        let result = retry_stopping_server(
            || {
                calls.set(calls.get() + 1);
                Ok(31)
            },
            |_| panic!("unexpected wait"),
        );
        assert_eq!(result.unwrap(), 31);
        assert_eq!(calls.get(), 1);
    }
}
