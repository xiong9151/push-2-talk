use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use rodio::source::SamplesBuffer;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

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

/// 初始化环回监听的 Sink
pub fn init_loopback_sink() {
    if let Some(handle) = STREAM_HANDLE.get() {
        match Sink::try_new(handle) {
            Ok(sink) => {
                sink.set_volume(0.1); // 较低音量，避免啸叫
                let mut guard = LOOPBACK_SINK.lock().unwrap();
                if let Some(ref old) = *guard {
                    old.stop();
                }
                *guard = Some(sink);
                tracing::info!("环回监听 Sink 已初始化, 音量=0.1");
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
pub fn loopback_play(samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mut guard = LOOPBACK_SINK.lock().unwrap();
    if let Some(ref sink) = *guard {
        let source = SamplesBuffer::new(1, 16000, samples.to_vec());
        sink.append(source);
        // 每 100 次打一次日志确认环回正在工作
        let prev = LOOPBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
        if prev % 100 == 0 {
            tracing::info!("环回监听: 已播放 {} 块, 当前块 {} 样本", prev + 1, samples.len());
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