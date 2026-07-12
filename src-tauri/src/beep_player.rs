use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::time::Duration;

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