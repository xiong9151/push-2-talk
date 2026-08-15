//! SpeexDSP acoustic echo cancellation (AEC) module.
//!
//! Vendored C source (SpeexDSP-1.2.1) compiled via the `cc` crate in
//! `build.rs`, gated behind the `aec` cargo feature. No pkg-config, no
//! system libspeexdsp — fully static, CI-friendly (OBS-style).

pub mod ffi;
pub mod processor;

/// AEC frame size in samples at 16 kHz = 10 ms.
/// SpeexDSP recommends 10-20 ms; this also aligns with RNNoise's 10 ms
/// frame boundary at 16 kHz.
pub const AEC_FRAME_SIZE: usize = 160;

/// AEC filter length (echo tail) in samples. 1600 = 100 ms at 16 kHz,
/// suitable for a small-to-medium room. Increase to 3200 for larger rooms.
pub const AEC_FILTER_LENGTH: usize = 1600;

/// Target sample rate for the AEC (near and far must be identical).
pub const AEC_SAMPLE_RATE: u32 = 16000;

/// Linear-interpolation resampler (matches the recorders' existing logic).
pub fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * ratio;
        let lo = src.floor() as usize;
        let hi = (lo + 1).min(input.len().saturating_sub(1));
        let frac = src - lo as f64;
        if lo < input.len() {
            let s = input[lo] as f64 * (1.0 - frac) + input[hi] as f64 * frac;
            out.push(s as f32);
        }
    }
    out
}

#[cfg(feature = "aec")]
/// Pull exactly `n` far-end samples from the loopback channel receiver,
/// topping up the local `accum` buffer. Zero-pads if starved (silence
/// reference = AEC no-op, safe — never distorts the near-end signal).
pub fn pull_far_samples(
    rx: &crossbeam_channel::Receiver<Vec<f32>>,
    accum: &mut Vec<f32>,
    n: usize,
) {
    // Top up until we have at least n, or the channel is empty.
    while accum.len() < n {
        match rx.try_recv() {
            Ok(chunk) => accum.extend_from_slice(&chunk),
            Err(_) => break,
        }
    }
    // If still short, pad with silence.
    while accum.len() < n {
        accum.push(0.0);
    }
}

/// Run AEC on a buffer of near-end samples, processing in fixed
/// `AEC_FRAME_SIZE` frames. `far_accum` is a local accumulator the
/// caller maintains across callbacks; we drain from it via the
/// provided `far_rx`. Mutates `near` in place.
#[cfg(feature = "aec")]
pub fn run_aec_realtime(
    aec: &mut crate::aec::processor::AecProcessor,
    near: &mut [f32],
    far_rx: &crossbeam_channel::Receiver<Vec<f32>>,
    far_accum: &mut Vec<f32>,
    frame_size: usize,
) {
    for chunk in near.chunks_mut(frame_size) {
        if chunk.len() < frame_size {
            // Partial tail frame — pad to frame_size for the C call,
            // then trim back. Rare at steady state.
            let mut padded = chunk.to_vec();
            padded.resize(frame_size, 0.0);
            pull_far_samples(far_rx, far_accum, frame_size);
            let far: Vec<f32> = far_accum.drain(..frame_size).collect();
            let mut near_frame = padded;
            aec.process_frame(&mut near_frame, &far);
            chunk.copy_from_slice(&near_frame[..chunk.len()]);
        } else {
            pull_far_samples(far_rx, far_accum, frame_size);
            let far: Vec<f32> = far_accum.drain(..frame_size).collect();
            aec.process_frame(chunk, &far);
        }
    }
}

/// Downmix multi-channel f32 to mono f32.
pub fn to_mono_f32(input: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return input.to_vec();
    }
    let ch = channels as usize;
    let out_len = input.len() / ch;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let mut sum = 0.0f32;
        for c in 0..ch {
            sum += input[i * ch + c];
        }
        out.push(sum / ch as f32);
    }
    out
}

/// Start a WASAPI loopback capture stream that pushes 16 kHz mono f32
/// chunks (the far-end / speaker reference) to `push`.
///
/// On Windows, calling `build_input_stream` on a render (output) device
/// makes cpal set `AUDCLNT_STREAMFLAGS_LOOPBACK` internally — so this
/// captures whatever the system is playing through the default speakers.
#[cfg(target_os = "windows")]
pub fn start_loopback_capture<F>(push: F) -> anyhow::Result<cpal::Stream>
where
    F: Fn(Vec<f32>) + Send + 'static,
{
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("没有找到默认音频输出设备（loopback）"))?;

    // A render endpoint's default *input* config gives the loopback format.
    let supported = match device.default_input_config() {
        Ok(s) => s,
        Err(_) => {
            // Fallback: assume stereo 48 kHz (typical WASAPI shared mode).
            // cpal::SupportedStreamConfig::new(channels, sample_rate, buffer_size, sample_format)
            cpal::SupportedStreamConfig::new(
                2,
                cpal::SampleRate(48000),
                cpal::SupportedBufferSize::Unknown,
                cpal::SampleFormat::F32,
            )
        }
    };
    let config = supported.config();
    let dev_rate = config.sample_rate.0;
    let channels = config.channels;

    let err_fn = |err| tracing::error!("[loopback] 录音流错误: {}", err);

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _info: &cpal::InputCallbackInfo| {
            let mono = to_mono_f32(data, channels);
            let resampled = resample_linear(&mono, dev_rate, AEC_SAMPLE_RATE);
            push(resampled);
        },
        err_fn,
        None,
    )?;
    stream.play()?;
    tracing::info!("[loopback] 环回采集已启动 ({}Hz, {}ch -> {}Hz mono)", dev_rate, channels, AEC_SAMPLE_RATE);
    Ok(stream)
}
