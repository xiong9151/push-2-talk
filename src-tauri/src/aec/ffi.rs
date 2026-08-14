//! Raw FFI bindings to vendored SpeexDSP echo cancellation.
//!
//! SpeexDSP-1.2.1 `mdf.c` exposes these symbols. Compiled in
//! FLOATING_POINT + USE_SMALLFT mode (see `vendor/speexdsp/config.h`).

#![allow(non_camel_case_types)]

use std::ffi::c_int;

/// SpeexDSP uses `spx_int16_t` for the echo API (always 16-bit PCM,
/// regardless of internal float/fixed representation).
pub type spx_int16_t = i16;
pub type spx_int32_t = i32;

/// Opaque echo canceller state.
#[repr(C)]
pub struct SpeexEchoState_;

// speex_echo_ctl request constants (from include/speex/speex_echo.h)
pub const SPEEX_ECHO_GET_FRAME_SIZE: c_int = 3;
pub const SPEEX_ECHO_SET_SAMPLING_RATE: c_int = 24;
pub const SPEEX_ECHO_GET_SAMPLING_RATE: c_int = 25;
pub const SPEEX_ECHO_GET_IMPULSE_RESPONSE_SIZE: c_int = 27;
pub const SPEEX_ECHO_GET_IMPULSE_RESPONSE: c_int = 29;

extern "C" {
    pub fn speex_echo_state_init(
        frame_size: c_int,
        filter_length: c_int,
    ) -> *mut SpeexEchoState_;
    pub fn speex_echo_state_destroy(st: *mut SpeexEchoState_);
    pub fn speex_echo_state_reset(st: *mut SpeexEchoState_);
    /// Core single-call AEC: takes near-end (`rec`) and far-end (`play`)
    /// frames, both exactly `frame_size` samples, writes cleaned `out`.
    pub fn speex_echo_cancellation(
        st: *mut SpeexEchoState_,
        rec: *const spx_int16_t,
        play: *const spx_int16_t,
        out: *mut spx_int16_t,
    );
    /// Split-path: queue a far-end frame. Must precede the matching
    /// `speex_echo_capture`. The capture path adds a 2-frame delay.
    pub fn speex_echo_playback(st: *mut SpeexEchoState_, play: *const spx_int16_t);
    /// Split-path: process a near-end frame against the oldest queued
    /// far-end frame. Requires a prior `speex_echo_playback`.
    pub fn speex_echo_capture(st: *mut SpeexEchoState_, rec: *const spx_int16_t, out: *mut spx_int16_t);
    pub fn speex_echo_ctl(
        st: *mut SpeexEchoState_,
        request: c_int,
        ptr: *mut std::ffi::c_void,
    ) -> c_int;
}
