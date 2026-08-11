use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;

// 在编译时嵌入提示音 WAV 文件
const START_BEEP: &[u8] = include_bytes!("../resources/start_beep.wav");
const STOP_BEEP: &[u8] = include_bytes!("../resources/stop_beep.wav");
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)
const VOLUME: f32 = 0.2;

/// 预初始化的音频输出句柄（Send + Sync，可安全跨线程使用）
static STREAM_HANDLE: std::sync::OnceLock<OutputStreamHandle> = std::sync::OnceLock::new();

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