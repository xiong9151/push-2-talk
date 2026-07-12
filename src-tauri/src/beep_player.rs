use rodio::{OutputStream, Sink, Source};
use std::io::Cursor;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)，调小这个值可以降低音量
const VOLUME: f32 = 0.2;

/// 播放提示音（非阻塞）
pub fn play_notification() {
    std::thread::spawn(|| {
        if let Err(e) = play_notification_blocking() {
            tracing::error!("播放提示音失败: {}", e);
        }
    });
}

/// 阻塞式播放提示音
fn play_notification_blocking() -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let cursor = Cursor::new(NOTIFICATION_SOUND);
    let source = rodio::Decoder::new(cursor)?.amplify(VOLUME);
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// 播放"开始录音"提示音 — 上升音 440Hz→880Hz
pub fn play_start_beep() {
    std::thread::spawn(|| {
        if let Err(e) = play_dual_tone(440.0, 880.0) {
            tracing::error!("播放开始提示音失败: {}", e);
        }
    });
}

/// 播放"停止录音"提示音 — 下降音 880Hz→440Hz
pub fn play_stop_beep() {
    std::thread::spawn(|| {
        if let Err(e) = play_dual_tone(880.0, 440.0) {
            tracing::error!("播放停止提示音失败: {}", e);
        }
    });
}

/// 播放两个连续的单音，用于区分开始/结束
fn play_dual_tone(first_hz: f32, second_hz: f32) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let tone1 = rodio::source::SineWave::new(first_hz)
        .take_duration(Duration::from_millis(100))
        .amplify(VOLUME);
    let tone2 = rodio::source::SineWave::new(second_hz)
        .take_duration(Duration::from_millis(100))
        .amplify(VOLUME);

    sink.append(tone1);
    sink.append(tone2);
    sink.sleep_until_end();

    Ok(())
}