//! Safe Rust wrapper around SpeexDSP echo cancellation.
//!
//! `AecProcessor` holds an opaque `SpeexEchoState`, preallocates i16
//! scratch buffers (so the cpal audio callback stays allocation-free),
//! and exposes a single mutating `process_frame(near, far)` that does
//! f32→i16→speex_echo_cancellation→i16→f32 in place.
//!
//! # Safety / threading
//! The underlying `SpeexEchoState` is NOT thread-safe. Access is
//! serialized by the caller (the recorder holds it behind a Mutex).
//! `unsafe impl Send` is sound because the processor is only ever
//! touched from one thread at a time.

use super::ffi::{
    speex_echo_cancellation, speex_echo_ctl, speex_echo_state_destroy, speex_echo_state_init,
    SpeexEchoState_, SPEEX_ECHO_SET_SAMPLING_RATE,
};
use super::{AEC_FILTER_LENGTH, AEC_FRAME_SIZE, AEC_SAMPLE_RATE};
use std::ffi::c_int;
use std::os::raw::c_void;

pub struct AecProcessor {
    state: *mut SpeexEchoState_,
    frame_size: usize,
    // Preallocated scratch (i16) — avoids per-frame allocation in the
    // realtime audio callback.
    near_i16: Vec<i16>,
    far_i16: Vec<i16>,
    out_i16: Vec<i16>,
}

impl AecProcessor {
    /// Create with default frame/filter sizes (160 / 1600 @ 16 kHz).
    pub fn new() -> Self {
        Self::with_params(AEC_FRAME_SIZE, AEC_FILTER_LENGTH, AEC_SAMPLE_RATE)
    }

    /// Create with explicit parameters.
    pub fn with_params(frame_size: usize, filter_length: usize, sample_rate: u32) -> Self {
        let state = unsafe {
            let st = speex_echo_state_init(frame_size as c_int, filter_length as c_int);
            assert!(!st.is_null(), "speex_echo_state_init returned NULL");
            let rate = sample_rate as c_int;
            speex_echo_ctl(
                st,
                SPEEX_ECHO_SET_SAMPLING_RATE,
                &rate as *const c_int as *mut c_void,
            );
            st
        };
        tracing::info!(
            "AecProcessor created: frame_size={}, filter_length={}, rate={}",
            frame_size,
            filter_length,
            sample_rate
        );
        Self {
            state,
            frame_size,
            near_i16: vec![0i16; frame_size],
            far_i16: vec![0i16; frame_size],
            out_i16: vec![0i16; frame_size],
        }
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Run AEC on one frame. `near` is mutated in place (becomes the
    /// cleaned output). `far` is the reference (speaker) frame. Both
    /// must be exactly `frame_size` samples; if `far` is shorter it is
    /// zero-padded (silence reference = AEC no-op, safe).
    pub fn process_frame(&mut self, near: &mut [f32], far: &[f32]) {
        debug_assert_eq!(near.len(), self.frame_size, "near frame size mismatch");
        let fs = self.frame_size;

        // f32 -> i16 (×32768, clamp)
        for (i, &s) in near.iter().enumerate().take(fs) {
            self.near_i16[i] = (s * 32768.0).clamp(-32768.0, 32767.0) as i16;
        }
        // far may be shorter; zero-pad
        self.far_i16.iter_mut().for_each(|s| *s = 0);
        let far_n = far.len().min(fs);
        for (i, &s) in far.iter().enumerate().take(far_n) {
            self.far_i16[i] = (s * 32768.0).clamp(-32768.0, 32767.0) as i16;
        }

        unsafe {
            speex_echo_cancellation(
                self.state,
                self.near_i16.as_ptr(),
                self.far_i16.as_ptr(),
                self.out_i16.as_mut_ptr(),
            );
        }

        // i16 -> f32 (/32768), write back into `near`
        for i in 0..fs {
            near[i] = self.out_i16[i] as f32 / 32768.0;
        }
    }

    /// Reset filter weights (call before reusing across recordings).
    pub fn reset(&mut self) {
        unsafe { super::ffi::speex_echo_state_reset(self.state) };
    }
}

impl Drop for AecProcessor {
    fn drop(&mut self) {
        if !self.state.is_null() {
            unsafe { speex_echo_state_destroy(self.state) };
            self.state = std::ptr::null_mut();
        }
    }
}

// SAFETY: SpeexEchoState is not thread-safe, but access is serialized
// by the recorder's Mutex — only one thread touches the processor at a
// time. The pointer is never shared concurrently.
unsafe impl Send for AecProcessor {}
