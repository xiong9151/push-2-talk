// 音频录制模块
use anyhow::Result;
use cpal::Stream;
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::audio_utils::{apply_agc, calculate_audio_level, emit_audio_level, validate_audio};

// API 要求的目标采样率
const TARGET_SAMPLE_RATE: u32 = 16000;

/// 线程安全的 cpal::Stream 包装器
///
/// 将 `cpal::Stream` 的 `!Send` 限制隔离到此包装器中，避免对整个
/// `AudioRecorder` 使用 `unsafe impl Send`。
///
/// # Safety
///
/// cpal::Stream 在 Windows 平台（WASAPI）上可以安全地在任意线程 Drop。
/// 原始的 `Stream` 之所以不实现 `Send`，是因为某些平台（如 Linux ALSA）
/// 的音频句柄不允许跨线程 Drop。Windows WASAPI 没有此限制。
/// 本项目是 Windows-only 桌面应用，因此此实现是 sound 的。
struct SendStream {
    inner: Option<Stream>,
}

// SAFETY: Windows WASAPI 音频流句柄可以安全地在任意线程 Drop。
// 本项目仅为 Windows 平台设计，无需跨平台兼容。
unsafe impl Send for SendStream {}

impl SendStream {
    fn none() -> Self {
        Self { inner: None }
    }

    fn set(&mut self, stream: Stream) {
        self.inner = Some(stream);
    }

    fn clear(&mut self) {
        self.inner = None;
    }
}

pub struct AudioRecorder {
    device_sample_rate: u32, // 设备实际采样率
    channels: u16,
    audio_data: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    stream: SendStream, // 保存 stream 引用（Send 包装，安全跨线程 Drop）
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            device_sample_rate: 48000, // 默认值，会在 start_recording 时更新
            channels: 1,
            audio_data: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(Mutex::new(false)),
            stream: SendStream::none(),
        })
    }

    /// 将音频从设备采样率降采样到目标采样率 (16kHz)
    fn resample(&self, input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return input.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(input.len() - 1);
            let frac = src_idx - idx_floor as f64;

            // 线性插值
            let sample = input[idx_floor] as f64 * (1.0 - frac) + input[idx_ceil] as f64 * frac;
            output.push(sample as f32);
        }

        output
    }

    /// 将多声道音频转换为单声道
    fn to_mono(&self, input: &[f32], channels: u16) -> Vec<f32> {
        if channels == 1 {
            return input.to_vec();
        }

        let channels = channels as usize;
        let output_len = input.len() / channels;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                sum += input[i * channels + ch];
            }
            output.push(sum / channels as f32);
        }

        output
    }

    pub fn start_recording(&mut self, app_handle: Option<AppHandle>) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        tracing::info!("开始录音...");

        // 清空之前的音频数据
        self.audio_data.lock().unwrap_or_else(|e| e.into_inner()).clear();
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = true;

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有找到默认音频输入设备"))?;

        // 获取设备支持的配置
        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("无法获取默认音频配置: {}", e))?;

        tracing::info!("设备支持的配置: {:?}", supported_config);

        // 使用设备支持的配置
        let config = supported_config.config();

        // 更新采样率和声道为设备实际支持的值
        self.device_sample_rate = config.sample_rate.0;
        self.channels = config.channels;

        tracing::info!(
            "设备配置: 采样率={}Hz, 声道={}, 目标采样率={}Hz",
            self.device_sample_rate,
            self.channels,
            TARGET_SAMPLE_RATE
        );

        let audio_data = Arc::clone(&self.audio_data);
        let is_recording = Arc::clone(&self.is_recording);
        let err_fn = |err| tracing::error!("录音流错误: {}", err);

        // 基于时间的音频级别发送控制（目标 30-40Hz）
        use std::time::Instant;
        let last_emit_time: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));

        // 根据采样格式创建不同的 stream
        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let app_handle_f32 = app_handle.clone();
                let last_emit_time_f32 = Arc::clone(&last_emit_time);
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if *is_recording.lock().unwrap_or_else(|e| e.into_inner()) {
                            let mut buffer = audio_data.lock().unwrap_or_else(|e| e.into_inner());
                            buffer.extend_from_slice(data);

                            // 基于时间的音频级别发送（目标 ~30Hz，每 33ms 发送一次）
                            if let Some(ref app) = app_handle_f32 {
                                let mut last_emit = last_emit_time_f32.lock().unwrap_or_else(|e| e.into_inner());
                                if last_emit.elapsed().as_millis() >= 33 {
                                    let level = calculate_audio_level(data);
                                    emit_audio_level(app, level);
                                    *last_emit = Instant::now();
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let audio_data_i16 = Arc::clone(&audio_data);
                let is_recording_i16 = Arc::clone(&is_recording);
                let app_handle_i16 = app_handle.clone();
                let last_emit_time_i16 = Arc::clone(&last_emit_time);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if *is_recording_i16.lock().unwrap_or_else(|e| e.into_inner()) {
                            let mut buffer = audio_data_i16.lock().unwrap_or_else(|e| e.into_inner());
                            // 转换 i16 到 f32
                            let f32_data: Vec<f32> =
                                data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                            buffer.extend(&f32_data);

                            // 基于时间的音频级别发送（目标 ~30Hz）
                            if let Some(ref app) = app_handle_i16 {
                                let mut last_emit = last_emit_time_i16.lock().unwrap_or_else(|e| e.into_inner());
                                if last_emit.elapsed().as_millis() >= 33 {
                                    let level = calculate_audio_level(&f32_data);
                                    emit_audio_level(app, level);
                                    *last_emit = Instant::now();
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let audio_data_u16 = Arc::clone(&audio_data);
                let is_recording_u16 = Arc::clone(&is_recording);
                let app_handle_u16 = app_handle;
                let last_emit_time_u16 = Arc::clone(&last_emit_time);
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if *is_recording_u16.lock().unwrap_or_else(|e| e.into_inner()) {
                            let mut buffer = audio_data_u16.lock().unwrap_or_else(|e| e.into_inner());
                            // 转换 u16 到 f32
                            let f32_data: Vec<f32> = data
                                .iter()
                                .map(|&s| (s as f32 - 32768.0) / 32768.0)
                                .collect();
                            buffer.extend(&f32_data);

                            // 基于时间的音频级别发送（目标 ~30Hz）
                            if let Some(ref app) = app_handle_u16 {
                                let mut last_emit = last_emit_time_u16.lock().unwrap_or_else(|e| e.into_inner());
                                if last_emit.elapsed().as_millis() >= 33 {
                                    let level = calculate_audio_level(&f32_data);
                                    emit_audio_level(app, level);
                                    *last_emit = Instant::now();
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("不支持的采样格式")),
        };

        stream.play()?;

        // 保存 stream 引用，保持录音流活跃
        self.stream.set(stream);

        Ok(())
    }

    /// 停止录音并返回处理后的音频数据（16kHz 单声道 WAV 格式的字节数组）
    pub fn stop_recording_to_memory(&mut self) -> Result<Vec<u8>> {
        tracing::info!("停止录音...");

        // 停止录音
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = false;

        // Drop stream，停止音频流
        self.stream.clear();

        // 等待一小段时间确保所有数据都已写入
        std::thread::sleep(std::time::Duration::from_millis(100));

        let raw_audio = self.audio_data.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let original_len = raw_audio.len();

        // 1. 转换为单声道
        let mono_audio = self.to_mono(&raw_audio, self.channels);
        tracing::info!("转单声道: {} -> {} 样本", original_len, mono_audio.len());

        // 2. 降采样到 16kHz
        let mut resampled_audio =
            self.resample(&mono_audio, self.device_sample_rate, TARGET_SAMPLE_RATE);
        tracing::info!(
            "降采样: {}Hz -> {}Hz, {} -> {} 样本",
            self.device_sample_rate,
            TARGET_SAMPLE_RATE,
            mono_audio.len(),
            resampled_audio.len()
        );

        // 3. AGC 处理（按块处理以保持平滑）
        let mut current_gain = 1.0;
        for chunk in resampled_audio.chunks_mut(3200) {
            apply_agc(chunk, &mut current_gain);
        }

        // 4. 写入内存中的 WAV 格式
        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in resampled_audio.iter() {
                let amplitude =
                    (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                writer.write_sample(amplitude)?;
            }
            writer.finalize()?;
        }

        let wav_data = cursor.into_inner();
        tracing::info!(
            "音频已转换为内存 WAV: {} bytes, 采样率: {}Hz",
            wav_data.len(),
            TARGET_SAMPLE_RATE
        );

        // 5. 验证音频有效性（过滤误触和静音）
        validate_audio(&wav_data)?;

        Ok(wav_data)
    }

    /// 停止录音并返回内存中的 WAV 数据，同时收集诊断信息
    pub fn stop_recording_with_diagnostics(&mut self) -> Result<(Vec<u8>, AudioDiagnostics)> {
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.stream.clear();
        std::thread::sleep(std::time::Duration::from_millis(100));

        let raw_audio = self.audio_data.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let original_len = raw_audio.len();
        let duration_secs = original_len as f64 / self.device_sample_rate as f64;

        // 计算原始 RMS
        let raw_rms = if !raw_audio.is_empty() {
            let sum: f64 = raw_audio.iter().map(|&s| (s as f64).powi(2)).sum();
            (sum / raw_audio.len() as f64).sqrt() as f32
        } else {
            0.0
        };

        // 计算原始峰值
        let raw_peak = raw_audio.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // 转单声道
        let mono_audio = self.to_mono(&raw_audio, self.channels);
        // 降采样
        let mut resampled_audio =
            self.resample(&mono_audio, self.device_sample_rate, TARGET_SAMPLE_RATE);

        // AGC 处理
        let mut current_gain = 1.0;
        let mut gain_history = Vec::new();
        for chunk in resampled_audio.chunks_mut(3200) {
            apply_agc(chunk, &mut current_gain);
            gain_history.push(current_gain);
        }

        // 计算处理后 RMS
        let processed_rms = if !resampled_audio.is_empty() {
            let sum: f64 = resampled_audio.iter().map(|&s| (s as f64).powi(2)).sum();
            (sum / resampled_audio.len() as f64).sqrt() as f32
        } else {
            0.0
        };

        // 写入 WAV
        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in resampled_audio.iter() {
                let amplitude =
                    (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                writer.write_sample(amplitude)?;
            }
            writer.finalize()?;
        }
        let wav_data = cursor.into_inner();

        let diagnostics = AudioDiagnostics {
            duration_secs,
            device_sample_rate: self.device_sample_rate,
            target_sample_rate: TARGET_SAMPLE_RATE,
            channels: self.channels,
            raw_sample_count: original_len,
            raw_rms,
            raw_peak,
            processed_rms,
            final_gain: current_gain,
            gain_history,
            wav_size_bytes: wav_data.len(),
        };

        Ok((wav_data, diagnostics))
    }

    /// 停止录音并保存到文件（保留兼容性，未使用）
    #[allow(dead_code)]
    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        tracing::info!("停止录音...");

        // 停止录音
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = false;

        // Drop stream，停止音频流
        self.stream.clear();

        // 等待一小段时间确保所有数据都已写入
        std::thread::sleep(std::time::Duration::from_millis(100));

        let raw_audio = self.audio_data.lock().unwrap_or_else(|e| e.into_inner()).clone();

        // 1. 转换为单声道
        let mono_audio = self.to_mono(&raw_audio, self.channels);

        // 2. 降采样到 16kHz
        let mut resampled_audio =
            self.resample(&mono_audio, self.device_sample_rate, TARGET_SAMPLE_RATE);

        // 3. AGC 处理（按块处理以保持平滑）
        let mut current_gain = 1.0;
        for chunk in resampled_audio.chunks_mut(3200) {
            apply_agc(chunk, &mut current_gain);
        }

        // 保存音频文件
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let file_path = temp_dir.join(format!("recording_{}.wav", timestamp));

        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&file_path, spec)?;

        for &sample in resampled_audio.iter() {
            let amplitude =
                (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer.write_sample(amplitude)?;
        }

        writer.finalize()?;
        tracing::info!(
            "音频已保存到: {:?}, 采样率: {}Hz",
            file_path,
            TARGET_SAMPLE_RATE
        );

        Ok(file_path)
    }

    /// 检查是否正在录音
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// 音频诊断信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDiagnostics {
    /// 录音时长（秒）
    pub duration_secs: f64,
    /// 设备采样率
    pub device_sample_rate: u32,
    /// 目标采样率
    pub target_sample_rate: u32,
    /// 声道数
    pub channels: u16,
    /// 原始样本数
    pub raw_sample_count: usize,
    /// 原始音频 RMS
    pub raw_rms: f32,
    /// 原始音频峰值
    pub raw_peak: f32,
    /// AGC 处理后音频 RMS
    pub processed_rms: f32,
    /// 最终增益
    pub final_gain: f32,
    /// 增益历史（每 3200 样本记录一次）
    pub gain_history: Vec<f32>,
    /// WAV 文件大小（字节）
    pub wav_size_bytes: usize,
}
