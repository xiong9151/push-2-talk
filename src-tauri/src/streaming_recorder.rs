// 流式音频录制模块
// 支持边录音边发送 PCM 数据块到 WebSocket

use anyhow::Result;
use cpal::Stream;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::AppHandle;

use crate::audio_utils::{
    apply_agc, calculate_audio_level, emit_audio_level, is_voice_active, validate_audio,
    NoiseReducer, AudioProcessConfig,
};

// API 要求的目标采样率
const TARGET_SAMPLE_RATE: u32 = 16000;
// 每个音频块的样本数（0.2秒 @ 16kHz = 3200 样本）
const CHUNK_SAMPLES: usize = 3200;

/// 流式音频录制器
/// 边录音边输出 PCM 数据块，同时保留完整音频用于备用方案
pub struct StreamingRecorder {
    device_sample_rate: u32,
    channels: u16,
    is_recording: Arc<Mutex<bool>>,
    stream: Option<Stream>,
    // 用于流式输出的通道
    chunk_sender: Option<Sender<Vec<i16>>>,
    // 累积的完整音频数据（用于备用方案）
    full_audio_data: Arc<Mutex<Vec<f32>>>,
    // 回调端数据写入完成信号，防止 stop_streaming 与回调的 race
    data_written: Arc<AtomicBool>,
    // RNNoise 降噪器（通过 Arc<Mutex> 供回调使用）
    noise_reducer: Arc<Mutex<Option<NoiseReducer>>>,
    // 环回监听开关：开启后将处理后的音频实时播放到扬声器
    loopback_enabled: Arc<AtomicBool>,
    // SpeexDSP AEC 处理器（仅在 enable_aec 时初始化）
    #[cfg(feature = "aec")]
    aec_state: Arc<Mutex<Option<crate::aec::processor::AecProcessor>>>,
    // 远端（扬声器环回）音频块通道：loopback 回调 push，mic 回调 pull
    #[cfg(feature = "aec")]
    far_end_rx: Arc<Mutex<Option<Arc<Receiver<Vec<f32>>>>>>,
    // loopback 捕获流（保持存活）
    #[cfg(feature = "aec")]
    loopback_stream: Arc<Mutex<Option<Stream>>>,
}

impl StreamingRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            device_sample_rate: 48000,
            channels: 1,
            is_recording: Arc::new(Mutex::new(false)),
            stream: None,
            chunk_sender: None,
            full_audio_data: Arc::new(Mutex::new(Vec::new())),
            data_written: Arc::new(AtomicBool::new(false)),
            noise_reducer: Arc::new(Mutex::new(None)),
            loopback_enabled: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "aec")]
            aec_state: Arc::new(Mutex::new(None)),
            #[cfg(feature = "aec")]
            far_end_rx: Arc::new(Mutex::new(None)),
            #[cfg(feature = "aec")]
            loopback_stream: Arc::new(Mutex::new(None)),
        })
    }

    /// 将音频从设备采样率降采样到目标采样率 (16kHz)
    fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return input.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(input.len().saturating_sub(1));
            let frac = src_idx - idx_floor as f64;

            if idx_floor < input.len() {
                let sample = input[idx_floor] as f64 * (1.0 - frac)
                    + input.get(idx_ceil).copied().unwrap_or(0.0) as f64 * frac;
                output.push(sample as f32);
            }
        }

        output
    }

    /// 将多声道音频转换为单声道
    fn to_mono(input: &[f32], channels: u16) -> Vec<f32> {
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

    /// 将 f32 样本转换为 i16
    fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
            .collect()
    }

    /// 音频处理配置设置（RNNoise 降噪 + 环回监听 + AEC）
    pub fn set_audio_processing_config(&mut self, config: &AudioProcessConfig) {
        let mut reducer = self.noise_reducer.lock().unwrap_or_else(|e| e.into_inner());
        if config.noise_reduction_strength > 0 {
            *reducer = Some(NoiseReducer::new(config.noise_reduction_strength as f32 / 100.0));
        } else {
            *reducer = None;
        }
        self.loopback_enabled.store(config.enable_loopback, Ordering::Release);
        tracing::info!(
            "降噪配置已更新: strength={}, loopback={}",
            config.noise_reduction_strength,
            config.enable_loopback
        );

        #[cfg(feature = "aec")]
        {
            use crate::aec::processor::AecProcessor;
            let mut aec = self.aec_state.lock().unwrap_or_else(|e| e.into_inner());
            if config.enable_aec && config.enable_loopback {
                *aec = Some(AecProcessor::new());
                tracing::info!(
                    "AEC 已启用: frame_size={}, filter_length={}",
                    crate::aec::AEC_FRAME_SIZE,
                    crate::aec::AEC_FILTER_LENGTH
                );
            } else {
                if aec.is_some() {
                    aec.take();
                }
                if config.enable_aec && !config.enable_loopback {
                    tracing::warn!("AEC 已开启但环回监听未开启 — AEC 无参考信号，将作为 no-op");
                }
            }
        }
    }

    /// 启动流式录音，返回音频块接收通道
    /// app_handle 用于发送音频级别事件到前端
    pub fn start_streaming(&mut self, app_handle: Option<AppHandle>) -> Result<Receiver<Vec<i16>>> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        // 添加降噪器引用用于回调闭包（在 self.noise_reducer 上 clone）
        let noise_reducer_clone = Arc::clone(&self.noise_reducer);
        // 环回监听开关引用
        let loopback_enabled_clone = Arc::clone(&self.loopback_enabled);

        tracing::info!("开始流式录音...");

        // 如果环回监听已开启，初始化 Sink 准备播放
        if self.loopback_enabled.load(Ordering::Acquire) {
            crate::beep_player::init_loopback_sink();
            tracing::info!("环回监听已初始化");
        }

        // 清空之前的数据
        self.full_audio_data.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.data_written.store(false, Ordering::Release);
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = true;

        // 创建音频块通道（缓冲 500 个块，约 100 秒，远超任何 ASR 超时）
        let (chunk_tx, chunk_rx) = bounded::<Vec<i16>>(500);
        self.chunk_sender = Some(chunk_tx.clone());

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有找到默认音频输入设备"))?;

        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("无法获取默认音频配置: {}", e))?;

        let config = supported_config.config();
        self.device_sample_rate = config.sample_rate.0;
        self.channels = config.channels;

        tracing::info!(
            "流式录音配置: 采样率={}Hz, 声道={}, 目标采样率={}Hz, 块大小={}样本",
            self.device_sample_rate,
            self.channels,
            TARGET_SAMPLE_RATE,
            CHUNK_SAMPLES
        );

        let is_recording = Arc::clone(&self.is_recording);
        let full_audio_data = Arc::clone(&self.full_audio_data);
        let data_written = Arc::clone(&self.data_written);
        let device_sample_rate = self.device_sample_rate;
        let channels = self.channels;

        // 用于累积样本直到达到块大小
        let pending_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let pending_samples_clone = Arc::clone(&pending_samples);

        // 基于时间的音频级别发送控制（目标 30-40Hz）
        use std::time::Instant;
        let last_emit_time: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
        let last_emit_time_clone = Arc::clone(&last_emit_time);
        let emit_counter: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let emit_counter_clone = Arc::clone(&emit_counter);

        // VAD 拖尾计数器：检测到静音后继续发送几个块，防止句尾吞字
        let vad_hangover: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let vad_hangover_clone = Arc::clone(&vad_hangover);
        const HANGOVER_CHUNKS: usize = 3; // 3块 * 0.2s = 0.6秒拖尾，平衡防吞字和响应速度

        // AGC 增益状态，用于平滑过渡
        let agc_gain: Arc<Mutex<f32>> = Arc::new(Mutex::new(1.0));
        let agc_gain_clone = Arc::clone(&agc_gain);

        // 降噪器引用（供 F32 回调使用）
        let noise_reducer_clone_f32 = Arc::clone(&noise_reducer_clone);
        // 环回监听开关引用（已在 start_streaming 顶部克隆）

        // AEC 相关引用（仅 aec feature 下）
        #[cfg(feature = "aec")]
        let aec_state_clone = Arc::clone(&self.aec_state);
        #[cfg(feature = "aec")]
        let far_end_rx_clone = Arc::clone(&self.far_end_rx);
        #[cfg(feature = "aec")]
        let aec_frame_size = crate::aec::AEC_FRAME_SIZE;

        // 克隆 app_handle 用于闭包
        let app_handle_f32 = app_handle.clone();

        let err_fn = |err| tracing::error!("录音流错误: {}", err);

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !*is_recording.lock().unwrap_or_else(|e| e.into_inner()) {
                        return;
                    }

                    // 保存原始数据用于备用方案
                    full_audio_data.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(data);
                    data_written.store(true, Ordering::Release);

                    // 处理数据：转单声道 + 降采样
                    let mono = Self::to_mono(data, channels);
                    let mut resampled = Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                    // === AEC 回声消除（在 VAD/AGC/RNNoise 之前）===
                    #[cfg(feature = "aec")]
                    {
                        if let Some(ref rx_opt) = *far_end_rx_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                            if let Some(ref mut aec) = *aec_state_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                                // 用一个临时累加器收集远端样本（每次回调新建，
                                // 前后帧的远端样本由通道本身保序）。
                                let mut far_accum: Vec<f32> = Vec::with_capacity(aec_frame_size);
                                crate::aec::run_aec_realtime(aec, &mut resampled, rx_opt, &mut far_accum, aec_frame_size);
                            }
                        }
                    }

                    // 基于时间的音频级别发送（目标 ~30Hz，每 33ms 发送一次）
                    if let Some(ref app) = app_handle_f32 {
                        let mut last_emit = last_emit_time_clone.lock().unwrap_or_else(|e| e.into_inner());
                        if last_emit.elapsed().as_millis() >= 33 {
                            let level = calculate_audio_level(&resampled);
                            emit_audio_level(app, level);
                            *last_emit = Instant::now();

                            // 调试日志：每30次打印一次（约每秒）
                            let mut counter = emit_counter_clone.lock().unwrap_or_else(|e| e.into_inner());
                            *counter += 1;
                            if *counter % 30 == 0 {
                                tracing::info!("[AudioLevel] 发送音频级别: {:.4} (30Hz)", level);
                            }
                        }
                    }

                    // 累积样本
                    let mut pending = pending_samples_clone.lock().unwrap_or_else(|e| e.into_inner());
                    pending.extend(resampled);

                    // 当累积足够样本时，发送块
                    while pending.len() >= CHUNK_SAMPLES {
                        let mut chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();

                        // VAD 判断
                        let is_active = is_voice_active(&chunk);
                        let mut hangover = vad_hangover_clone.lock().unwrap_or_else(|e| e.into_inner());

                        if is_active {
                            *hangover = HANGOVER_CHUNKS;
                        } else if *hangover > 0 {
                            *hangover -= 1;
                        }

                        // 静音且拖尾结束，丢弃前先衰减增益
                        if !is_active && *hangover == 0 {
                            let mut gain = agc_gain_clone.lock().unwrap_or_else(|e| e.into_inner());
                            *gain = *gain * 0.5 + 0.5;
                            continue;
                        }
                        drop(hangover);

                        // AGC（带平滑处理）
                        let mut gain = agc_gain_clone.lock().unwrap_or_else(|e| e.into_inner());
                        apply_agc(&mut chunk, &mut gain);
                        drop(gain);

                        // RNNoise 降噪处理
                        if let Some(ref mut reducer) = *noise_reducer_clone_f32.lock().unwrap_or_else(|e| e.into_inner()) {
                            let denoised = reducer.process(&chunk);
                            // 防止 denoised 比 chunk 短时 copy_from_slice panic
                            let copy_len = chunk.len().min(denoised.len());
                            chunk[..copy_len].copy_from_slice(&denoised[..copy_len]);
                        }

                        // 环回监听：将处理后的音频实时播放到扬声器
                        if loopback_enabled_clone.load(Ordering::Acquire) {
                            crate::beep_player::loopback_play(&chunk);
                        }

                        let chunk_i16 = Self::f32_to_i16(&chunk);

                        if chunk_tx.try_send(chunk_i16).is_err() {
                            tracing::warn!("音频块通道已满，丢弃块");
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => {
                let is_recording_i16 = Arc::clone(&is_recording);
                let full_audio_data_i16 = Arc::clone(&full_audio_data);
                let data_written_i16 = Arc::clone(&data_written);
                let pending_samples_i16 = Arc::clone(&pending_samples);
                let chunk_tx_i16 = chunk_tx.clone();
                let last_emit_time_i16 = Arc::clone(&last_emit_time);
                let app_handle_i16 = app_handle.clone();
                let vad_hangover_i16 = Arc::clone(&vad_hangover);
                let agc_gain_i16 = Arc::clone(&agc_gain);
                let loopback_enabled_i16 = Arc::clone(&self.loopback_enabled);

                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !*is_recording_i16.lock().unwrap_or_else(|e| e.into_inner()) {
                            return;
                        }

                        // 转换为 f32
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();

                        // 保存原始数据
                        full_audio_data_i16.lock().unwrap_or_else(|e| e.into_inner()).extend(&f32_data);
                        data_written_i16.store(true, Ordering::Release);

                        // 处理数据
                        let mono = Self::to_mono(&f32_data, channels);
                        let mut resampled =
                            Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                        // === AEC 回声消除 ===
                        #[cfg(feature = "aec")]
                        {
                            if let Some(ref rx_opt) = *far_end_rx_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                                if let Some(ref mut aec) = *aec_state_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                                    let mut far_accum: Vec<f32> = Vec::with_capacity(aec_frame_size);
                                    crate::aec::run_aec_realtime(aec, &mut resampled, rx_opt, &mut far_accum, aec_frame_size);
                                }
                            }
                        }

                        // 基于时间的音频级别发送（目标 ~30Hz）
                        if let Some(ref app) = app_handle_i16 {
                            let mut last_emit = last_emit_time_i16.lock().unwrap_or_else(|e| e.into_inner());
                            if last_emit.elapsed().as_millis() >= 33 {
                                let level = calculate_audio_level(&resampled);
                                emit_audio_level(app, level);
                                *last_emit = Instant::now();
                            }
                        }

                        // 累积样本
                        let mut pending = pending_samples_i16.lock().unwrap_or_else(|e| e.into_inner());
                        pending.extend(resampled);

                        while pending.len() >= CHUNK_SAMPLES {
                            let mut chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();

                            // VAD 判断
                            let is_active = is_voice_active(&chunk);
                            let mut hangover = vad_hangover_i16.lock().unwrap_or_else(|e| e.into_inner());

                            if is_active {
                                *hangover = HANGOVER_CHUNKS;
                            } else if *hangover > 0 {
                                *hangover -= 1;
                            }

                            // 静音且拖尾结束，丢弃前先衰减增益
                            if !is_active && *hangover == 0 {
                                let mut gain = agc_gain_i16.lock().unwrap_or_else(|e| e.into_inner());
                                *gain = *gain * 0.5 + 0.5;
                                continue;
                            }
                            drop(hangover);

                            // AGC（带平滑处理）
                            let mut gain = agc_gain_i16.lock().unwrap_or_else(|e| e.into_inner());
                            apply_agc(&mut chunk, &mut gain);
                            drop(gain);

                            // 环回监听：将处理后的音频实时播放到扬声器
                            if loopback_enabled_i16.load(Ordering::Acquire) {
                                crate::beep_player::loopback_play(&chunk);
                            }

                            let chunk_i16 = Self::f32_to_i16(&chunk);

                            if chunk_tx_i16.try_send(chunk_i16).is_err() {
                                tracing::warn!("音频块通道已满，丢弃块");
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let is_recording_u16 = Arc::clone(&is_recording);
                let full_audio_data_u16 = Arc::clone(&full_audio_data);
                let data_written_u16 = Arc::clone(&data_written);
                let pending_samples_u16 = Arc::clone(&pending_samples);
                let chunk_tx_u16 = chunk_tx.clone();
                let last_emit_time_u16 = Arc::clone(&last_emit_time);
                let app_handle_u16 = app_handle;
                let vad_hangover_u16 = Arc::clone(&vad_hangover);
                let agc_gain_u16 = Arc::clone(&agc_gain);
                let loopback_enabled_u16 = Arc::clone(&self.loopback_enabled);

                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if !*is_recording_u16.lock().unwrap_or_else(|e| e.into_inner()) {
                            return;
                        }

                        // 转换为 f32
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();

                        // 保存原始数据
                        full_audio_data_u16.lock().unwrap_or_else(|e| e.into_inner()).extend(&f32_data);
                        data_written_u16.store(true, Ordering::Release);

                        // 处理数据
                        let mono = Self::to_mono(&f32_data, channels);
                        let mut resampled =
                            Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                        // === AEC 回声消除 ===
                        #[cfg(feature = "aec")]
                        {
                            if let Some(ref rx_opt) = *far_end_rx_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                                if let Some(ref mut aec) = *aec_state_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                                    let mut far_accum: Vec<f32> = Vec::with_capacity(aec_frame_size);
                                    crate::aec::run_aec_realtime(aec, &mut resampled, rx_opt, &mut far_accum, aec_frame_size);
                                }
                            }
                        }

                        // 基于时间的音频级别发送（目标 ~30Hz）
                        if let Some(ref app) = app_handle_u16 {
                            let mut last_emit = last_emit_time_u16.lock().unwrap_or_else(|e| e.into_inner());
                            if last_emit.elapsed().as_millis() >= 33 {
                                let level = calculate_audio_level(&resampled);
                                emit_audio_level(app, level);
                                *last_emit = Instant::now();
                            }
                        }

                        // 累积样本
                        let mut pending = pending_samples_u16.lock().unwrap_or_else(|e| e.into_inner());
                        pending.extend(resampled);

                        while pending.len() >= CHUNK_SAMPLES {
                            let mut chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();

                            // VAD 判断
                            let is_active = is_voice_active(&chunk);
                            let mut hangover = vad_hangover_u16.lock().unwrap_or_else(|e| e.into_inner());

                            if is_active {
                                *hangover = HANGOVER_CHUNKS;
                            } else if *hangover > 0 {
                                *hangover -= 1;
                            }

                            // 静音且拖尾结束，丢弃前先衰减增益
                            if !is_active && *hangover == 0 {
                                let mut gain = agc_gain_u16.lock().unwrap_or_else(|e| e.into_inner());
                                *gain = *gain * 0.5 + 0.5;
                                continue;
                            }
                            drop(hangover);

                            // AGC（带平滑处理）
                            let mut gain = agc_gain_u16.lock().unwrap_or_else(|e| e.into_inner());
                            apply_agc(&mut chunk, &mut gain);
                            drop(gain);

                            // 环回监听：将处理后的音频实时播放到扬声器
                            if loopback_enabled_u16.load(Ordering::Acquire) {
                                crate::beep_player::loopback_play(&chunk);
                            }

                            let chunk_i16 = Self::f32_to_i16(&chunk);

                            if chunk_tx_u16.try_send(chunk_i16).is_err() {
                                tracing::warn!("音频块通道已满，丢弃块");
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("不支持的采样格式")),
        };

        // === 启动 loopback 环回采集（AEC 远端参考信号）===
        #[cfg(feature = "aec")]
        {
            let aec_active = self.aec_state.lock().unwrap_or_else(|e| e.into_inner()).is_some();
            if aec_active {
                let (far_tx, far_rx) = bounded::<Vec<f32>>(500);
                match crate::aec::start_loopback_capture(move |chunk| {
                    let _ = far_tx.try_send(chunk);
                }) {
                    Ok(lb_stream) => {
                        *self.loopback_stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(lb_stream);
                        *self.far_end_rx.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(far_rx));
                        tracing::info!("AEC loopback 环回采集已启动");
                    }
                    Err(e) => {
                        tracing::warn!("启动 loopback 环回采集失败，AEC 将以静音参考运行（no-op）: {}", e);
                        // far_end_rx 保持 None，回调会用静音填充，安全
                    }
                }
            }
        }

        stream.play()?;
        self.stream = Some(stream);

        tracing::info!("流式录音已启动");
        Ok(chunk_rx)
    }

    /// 停止流式录音，返回完整的音频数据（WAV 格式，用于备用方案）
    pub fn stop_streaming(&mut self) -> Result<Vec<u8>> {
        use hound::{WavSpec, WavWriter};
        use std::io::Cursor;

        tracing::info!("停止流式录音...");

        // 通知音频回调停止（回调在独立音频线程，需先让回调感知）
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner()) = false;

        // 等待音频线程完成最后一次数据写入（轮询 data_written 信号，带超时）
        let data_written = Arc::clone(&self.data_written);
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(1000);
        let poll_interval = std::time::Duration::from_millis(20);

        // 先等待一段固定短时间，让音频回调感知 is_recording=false
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 然后轮询 data_written，直到回调完成写入或超时
        loop {
            if data_written.load(Ordering::Acquire) {
                break;
            }
            if start.elapsed() >= timeout {
                tracing::warn!("等待音频回调数据写入超时，继续停止流程");
                break;
            }
            std::thread::sleep(poll_interval);
        }

        // 最后 drop stream
        self.stream = None;
        self.chunk_sender = None;

        // 停止环回监听播放
        crate::beep_player::stop_loopback();

        // 停止 loopback 环回采集流（AEC 远端）
        #[cfg(feature = "aec")]
        {
            *self.loopback_stream.lock().unwrap_or_else(|e| e.into_inner()) = None;
            *self.far_end_rx.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }

        // 获取完整音频数据
        let raw_audio = self.full_audio_data.lock().unwrap_or_else(|e| e.into_inner()).clone();

        if raw_audio.is_empty() {
            return Err(anyhow::anyhow!("没有录制到音频数据"));
        }

        // 转换为单声道
        let mono_audio = Self::to_mono(&raw_audio, self.channels);

        // 降采样到 16kHz
        let resampled_audio =
            Self::resample(&mono_audio, self.device_sample_rate, TARGET_SAMPLE_RATE);

        // 写入 WAV 格式
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
        tracing::info!("流式录音停止，完整音频: {} bytes", wav_data.len());

        // 验证音频有效性（过滤误触和静音）
        validate_audio(&wav_data)?;

        Ok(wav_data)
    }

    /// 检查是否正在录音
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// 实现 Send 和 Sync traits
unsafe impl Send for StreamingRecorder {}
unsafe impl Sync for StreamingRecorder {}
