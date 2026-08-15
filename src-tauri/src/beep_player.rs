use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// 在编译时嵌入提示音 WAV 文件
const START_BEEP: &[u8] = include_bytes!("../resources/start_beep.wav");
const STOP_BEEP: &[u8] = include_bytes!("../resources/stop_beep.wav");
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)
const VOLUME: f32 = 0.2;

/// 预初始化的音频输出句柄（Send + Sync，可安全跨线程使用）
static STREAM_HANDLE: std::sync::OnceLock<OutputStreamHandle> = std::sync::OnceLock::new();

/// 环回监听（实时回放处理后的音频到扬声器）
/// 使用独立的 Sink，在录音期间持续追加音频帧
static LOOPBACK_SINK: Mutex<Option<Sink>> = Mutex::new(None);
/// 环回监听播放计数器（调试用）
static LOOPBACK_COUNTER: AtomicU64 = AtomicU64::new(0);
/// 独立环回模式：不依赖录音，麦克风直接实时监听并回放
static LOOPBACK_STREAM_ON: AtomicBool = AtomicBool::new(false);

/// 启动独立麦克风监听环回 — 打开开关后实时采集麦克风，处理（降噪+AGC）后播放到扬声器
pub fn start_mic_monitor() {
    if LOOPBACK_STREAM_ON.load(Ordering::SeqCst) {
        return;
    }
    LOOPBACK_STREAM_ON.store(true, Ordering::SeqCst);
    // 初始化 Sink
    init_loopback_sink();
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                tracing::warn!("环回监听: 无麦克风设备");
                LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
                return;
            }
        };
        let supported_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("环回监听: 无法获取麦克风配置: {}", e);
                LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
                return;
            }
        };
        let config = supported_config.config();
        let device_rate = config.sample_rate.0;
        let channels = config.channels;
        let err_fn = |err| tracing::error!("环回监听流错误: {}", err);

        // 降噪器和 AGC 状态
        let mut reducer = crate::audio_utils::NoiseReducer::new(0.8);
        let mut agc_gain = 1.0f32;

        // 格式无关的原始数据收集（所有格式都先转成 f32 再统一处理）
        let raw_f32: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let is_recording: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let raw = Arc::clone(&raw_f32);
                let flag = Arc::clone(&is_recording);
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !flag.load(Ordering::SeqCst) { return; }
                        raw.lock().unwrap().extend_from_slice(data);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let raw = Arc::clone(&raw_f32);
                let flag = Arc::clone(&is_recording);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !flag.load(Ordering::SeqCst) { return; }
                        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        raw.lock().unwrap().extend(f32_data);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let raw = Arc::clone(&raw_f32);
                let flag = Arc::clone(&is_recording);
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if !flag.load(Ordering::SeqCst) { return; }
                        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                        raw.lock().unwrap().extend(f32_data);
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                tracing::warn!("环回监听: 不支持的采样格式");
                LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
                return;
            }
        };

        match stream {
            Ok(s) => {
                if let Err(e) = s.play() {
                    tracing::warn!("环回监听: 启动播放失败: {}", e);
                    LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
                    return;
                }
                tracing::info!("环回监听: 已启动, 采样率={}, 声道={}, 格式={:?}", device_rate, channels, supported_config.sample_format());
                // 处理循环：每 50ms 从 raw_f32 取数据，处理并播放
                let mut last_pos = 0usize;
                let mut pending: Vec<f32> = Vec::new();
                while LOOPBACK_STREAM_ON.load(Ordering::SeqCst) {
                    let chunk: Vec<f32> = {
                        let mut buf = raw_f32.lock().unwrap();
                        if buf.len() > last_pos + 3200 {
                            let c = buf[last_pos..last_pos + 3200].to_vec();
                            last_pos += 3200;
                            // 防止 buf 无限增长
                            if last_pos > 96000 {
                                buf.drain(0..last_pos);
                                last_pos = 0;
                            }
                            c
                        } else {
                            Vec::new()
                        }
                    };
                    if !chunk.is_empty() {
                        // 转单声道
                        let mono = if channels > 1 {
                            chunk.chunks(channels as usize)
                                .map(|ch| ch.iter().sum::<f32>() / channels as f32)
                                .collect::<Vec<_>>()
                        } else {
                            chunk
                        };
                        // 降采样到 16kHz
                        let ratio = device_rate as f64 / 16000.0;
                        let out_len = (mono.len() as f64 / ratio) as usize;
                        let mut resampled = Vec::with_capacity(out_len);
                        for i in 0..out_len {
                            let src = i as f64 * ratio;
                            let lo = src.floor() as usize;
                            let hi = (lo + 1).min(mono.len().saturating_sub(1));
                            let frac = src - lo as f64;
                            if lo < mono.len() {
                                resampled.push((mono[lo] as f64 * (1.0 - frac) + mono[hi] as f64 * frac) as f32);
                            }
                        }
                        // RNNoise 降噪优先（先去掉噪声，避免降噪把音量一起压小）
                        let mut denoised = Vec::with_capacity(resampled.len());
                        for c in resampled.chunks(3200) {
                            let d = reducer.process(c);
                            denoised.extend_from_slice(&d);
                        }
                        // AGC 补足音量
                        for c in denoised.chunks_mut(3200) {
                            crate::audio_utils::apply_agc(c, &mut agc_gain);
                        }
                        // 播放
                        loopback_play_inner(&denoised);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
                drop(s);
            }
            Err(e) => {
                tracing::warn!("环回监听: 创建监听流失败: {}", e);
            }
        }
        LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
        tracing::info!("环回监听: 已停止");
    });
}

/// 停止独立麦克风监听环回
pub fn stop_mic_monitor() {
    LOOPBACK_STREAM_ON.store(false, Ordering::SeqCst);
    stop_loopback();
    tracing::info!("环回监听: 已发送停止信号");
}

/// 初始化环回监听的 Sink
pub fn init_loopback_sink() {
    if let Some(handle) = STREAM_HANDLE.get() {
        match Sink::try_new(handle) {
            Ok(sink) => {
                sink.set_volume(0.4); // 适中音量（原 0.1 太小）
                let mut guard = LOOPBACK_SINK.lock().unwrap();
                if let Some(ref old) = *guard {
                    old.stop();
                }
                *guard = Some(sink);
                tracing::info!("环回监听 Sink 已初始化, 音量=0.4");
            }
            Err(e) => {
                tracing::warn!("环回监听 Sink 初始化失败: {}", e);
            }
        }
    } else {
        tracing::warn!("环回监听: STREAM_HANDLE 未初始化");
    }
}

/// 将一段处理后音频追加到环回监听播放队列
/// 编码为内存 WAV → rodio Decoder 播放（兼容性最好）
/// 如果 Sink 尚未初始化，自动初始化（支持运行时开启）
pub fn loopback_play(samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mut guard = LOOPBACK_SINK.lock().unwrap();
    // 如果 Sink 尚未初始化，自动初始化（支持运行时开启）
    if guard.is_none() {
        drop(guard);
        init_loopback_sink();
        guard = LOOPBACK_SINK.lock().unwrap();
    }
    if let Some(ref sink) = *guard {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav_data = Vec::with_capacity(44 + samples.len() * 2);
        {
            let cursor = std::io::Cursor::new(&mut wav_data);
            if let Ok(mut writer) = hound::WavWriter::new(cursor, spec) {
                for &s in samples {
                    let amp = (s * 32768.0).clamp(-32768.0, 32767.0) as i16;
                    let _ = writer.write_sample(amp);
                }
                let _ = writer.finalize();
            }
        }
        if let Ok(source) = rodio::Decoder::new(std::io::Cursor::new(wav_data)) {
            sink.append(source);
        }
        let prev = LOOPBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
        if prev % 100 == 0 {
            tracing::info!("环回监听: 已播放 {} 块, 当前块 {} 样本", prev + 1, samples.len());
        }
    }
}

/// 内部播放函数：不依赖外部队列锁（供独立环回模式使用）
fn loopback_play_inner(samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mut guard = LOOPBACK_SINK.lock().unwrap();
    if guard.is_none() {
        drop(guard);
        init_loopback_sink();
        guard = LOOPBACK_SINK.lock().unwrap();
    }
    if let Some(ref sink) = *guard {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav_data = Vec::with_capacity(44 + samples.len() * 2);
        {
            let cursor = std::io::Cursor::new(&mut wav_data);
            if let Ok(mut writer) = hound::WavWriter::new(cursor, spec) {
                for &s in samples {
                    let amp = (s * 32768.0).clamp(-32768.0, 32767.0) as i16;
                    let _ = writer.write_sample(amp);
                }
                let _ = writer.finalize();
            }
        }
        if let Ok(source) = rodio::Decoder::new(std::io::Cursor::new(wav_data)) {
            sink.append(source);
        }
    }
}

/// 停止环回监听播放
pub fn stop_loopback() {
    let mut guard = LOOPBACK_SINK.lock().unwrap();
    if let Some(ref sink) = *guard {
        sink.stop();
    }
    *guard = None;
    LOOPBACK_COUNTER.store(0, Ordering::Relaxed);
    tracing::info!("环回监听已停止");
}

/// 提前初始化音频输出句柄，消除首次按键延迟
pub fn preinit() {
    if STREAM_HANDLE.get().is_some() {
        return;
    }
    if let Ok((stream, handle)) = OutputStream::try_default() {
        Box::leak(Box::new(stream)); // 永久保持音频设备活跃
        let _ = STREAM_HANDLE.set(handle);
    }
}

/// 在后台线程中播放 WAV 音频数据
fn play_wav(data: &'static [u8], volume: f32) {
    if let Some(handle) = STREAM_HANDLE.get() {
        std::thread::spawn(move || {
            if let Ok(sink) = Sink::try_new(handle) {
                let cursor = Cursor::new(data);
                if let Ok(source) = rodio::Decoder::new(cursor) {
                    sink.append(source.amplify(volume));
                    sink.sleep_until_end();
                }
            }
        });
    }
}

/// 播放提示音（非阻塞）
pub fn play_notification() {
    play_wav(NOTIFICATION_SOUND, VOLUME);
}

/// 播放"开始录音"提示音 — 木琴升调：C5→E5→G5
pub fn play_start_beep() {
    play_wav(START_BEEP, 0.3);
}

/// 播放"停止录音"提示音 — 木琴降调：G5→E5→C5
pub fn play_stop_beep() {
    play_wav(STOP_BEEP, 0.3);
}

/// 播放 WAV 文件（用于录音诊断）
///
/// 注意：由于 rodio 的 OutputStream 必须保持存活才能播放，这个函数是同步的。
pub fn play_wav_file(path: &str) -> Result<(), String> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path).map_err(|e| format!("无法打开音频文件: {}", e))?;
    let source = rodio::Decoder::new(BufReader::new(file))
        .map_err(|e| format!("无法解码音频: {}", e))?;

    let (_stream, stream_handle) = rodio::OutputStream::try_default()
        .map_err(|e| format!("无法初始化音频输出: {}", e))?;

    stream_handle
        .play_raw(source.convert_samples())
        .map_err(|e| format!("无法播放音频: {}", e))?;

    // 等待播放完成
    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}

/// 播放音频缓冲区（f32 格式，16kHz）
/// 用于播放降噪后的音频试听
pub fn play_audio_buffer(samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav_data = Vec::new();
    {
        let mut writer =
            hound::WavWriter::new(Cursor::new(&mut wav_data), spec).map_err(|e| e.to_string())?;
        for &sample in samples {
            let amplitude = (sample * 32768.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(amplitude).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    let cursor = Cursor::new(wav_data);
    let (_stream, stream_handle) = rodio::OutputStream::try_default()
        .map_err(|e| e.to_string())?;
    let source = rodio::Decoder::new(cursor).map_err(|e| e.to_string())?;
    stream_handle
        .play_raw(source.convert_samples())
        .map_err(|e| e.to_string())?;
    // 等待播放完成
    std::thread::sleep(std::time::Duration::from_secs(5));
    Ok(())
}