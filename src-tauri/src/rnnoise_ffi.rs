//! Raw FFI bindings to the vendored RNNoise C library.
//!
//! RNNoise is a noise suppression library based on a recurrent neural network.
//! The C library works at 48 kHz with a fixed frame size of 480 samples (10 ms).

use std::ffi::c_int;

/// Opaque handle to a DenoiseState created by `rnnoise_create`.
///
/// The C library's `DenoiseState` is an internal struct; we only hold a
/// pointer to it.  Call `rnnoise_destroy` to free it.
#[repr(C)]
pub struct DenoiseState {
    _private: [u8; 0],
}

/// Opaque handle to a custom RNNModel loaded from file.
///
/// Pass `NULL` (i.e. `std::ptr::null_mut()`) to `rnnoise_create` to use the
/// built-in default model.
#[repr(C)]
pub struct RNNModel {
    _private: [u8; 0],
}

extern "C" {
    /// Return the size of `DenoiseState` in bytes.
    pub fn rnnoise_get_size() -> c_int;

    /// Return the number of samples processed by `rnnoise_process_frame` at a time.
    ///
    /// This is always 480 (10 ms at 48 kHz).
    pub fn rnnoise_get_frame_size() -> c_int;

    /// Initialise a pre-allocated `DenoiseState`.
    ///
    /// Pass `std::ptr::null_mut()` for `model` to use the built-in default model.
    pub fn rnnoise_init(st: *mut DenoiseState, model: *mut RNNModel) -> c_int;

    /// Allocate and initialise a `DenoiseState`.
    ///
    /// Pass `std::ptr::null_mut()` for `model` to use the built-in default model.
    /// The returned pointer MUST be freed with `rnnoise_destroy`.
    pub fn rnnoise_create(model: *mut RNNModel) -> *mut DenoiseState;

    /// Free a `DenoiseState` produced by `rnnoise_create`.
    pub fn rnnoise_destroy(st: *mut DenoiseState);

    /// Denoise one frame of audio.
    ///
    /// `in` and `out` must each be at least `rnnoise_get_frame_size()` (480) elements.
    /// Returns the VAD probability (0.0 = silence, 1.0 = speech).
    pub fn rnnoise_process_frame(st: *mut DenoiseState, out: *mut f32, inp: *const f32) -> f32;

    /// Load a custom model from a file.
    ///
    /// Must be deallocated with `rnnoise_model_free`.
    pub fn rnnoise_model_from_file(f: *mut std::ffi::c_void) -> *mut RNNModel;

    /// Free a custom model.
    ///
    /// Must be called after all `DenoiseState` referring to it are freed.
    pub fn rnnoise_model_free(model: *mut RNNModel);
}