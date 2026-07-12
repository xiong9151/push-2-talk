use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)
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

    let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
    let source = rodio::Decoder::new(cursor)?.amplify(VOLUME);
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// 播放"开始录音"提示音 — 第一声弱 + 第二声强（渐强）
pub fn play_start_beep() {
    std::thread::spawn(|| {
        if let Err(e) = play_dual_volume(0.08, 0.25) {
            tracing::error!("播放开始提示音失败: {}", e);
        }
    });
}

/// 播放"停止录音"提示音 — 第一声强 + 第二声弱（渐弱）
pub fn play_stop_beep() {
    std::thread::spawn(|| {
        if let Err(e) = play_dual_volume(0.25, 0.08) {
            tracing::error!("播放停止提示音失败: {}", e);
        }
    });
}

/// 播放两声 440Hz 提示音，每声 200ms，使用不同的音量区分开始/结束
fn play_dual_volume(tone1_vol: f32, tone2_vol: f32) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let tone1 = rodio::source::SineWave::new(440.0)
        .take_duration(Duration::from_millis(200))
        .amplify(tone1_vol);
    let tone2 = rodio::source::SineWave::new(440.0)
        .take_duration(Duration::from_millis(200))
        .amplify(tone2_vol);

    sink.append(tone1);
    sink.append(tone2);
    sink.sleep_until_end();

    Ok(())
}