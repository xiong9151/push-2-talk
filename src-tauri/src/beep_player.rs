use rodio::{OutputStream, Sink, Source};
use rodio::source::SineWave;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 预初始化音频播放器，确保首次按键时提示音无延迟
lazy_static::lazy_static! {
    static ref AUDIO_PLAYER: std::sync::Mutex<Option<(OutputStream, Sink)>> =
        std::sync::Mutex::new(None);
}

/// 获取或初始化音频播放器（惰性初始化，首次调用时创建）
fn get_player() -> std::sync::MutexGuard<'static, Option<(OutputStream, Sink)>> {
    let mut guard = AUDIO_PLAYER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        if let Ok((stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                *guard = Some((stream, sink));
            }
        }
    }
    guard
}

/// 提前初始化音频播放器（在应用启动时调用，消除首次按键的延迟）
pub fn preinit() {
    let _ = get_player();
}

/// 播放提示音（非阻塞）
pub fn play_notification() {
    std::thread::spawn(|| {
        let mut guard = get_player();
        if let Some((_, ref sink)) = *guard {
            let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
            if let Ok(source) = rodio::Decoder::new(cursor) {
                sink.clear();
                sink.append(source.amplify(0.2));
                sink.sleep_until_end();
            }
        }
    });
}

/// 播放"开始录音"提示音
///
/// 第 0 秒：短促的 50% 音量滴声（100ms）
/// 第 0.5 秒：短促的 100% 音量滴声（100ms）
pub fn play_start_beep() {
    std::thread::spawn(|| {
        let mut guard = get_player();
        if let Some((_, ref sink)) = *guard {
            sink.clear();
            let tone1 = SineWave::new(440.0)
                .take_duration(Duration::from_millis(100))
                .amplify(0.5);
            sink.append(tone1);
            // 释放锁，让音频回调能处理数据
            drop(guard);
            // 等待 400ms，使两声之间间隔 500ms（第一声 100ms + 400ms 静音）
            std::thread::sleep(Duration::from_millis(400));
            let mut guard = get_player();
            if let Some((_, ref sink)) = *guard {
                let tone2 = SineWave::new(440.0)
                    .take_duration(Duration::from_millis(100))
                    .amplify(1.0);
                sink.append(tone2);
                sink.sleep_until_end();
            }
        }
    });
}

/// 播放"停止录音"提示音
///
/// 第 0 秒：短促的 100% 音量滴声（100ms）
/// 第 0.5 秒：短促的 50% 音量滴声（100ms）
pub fn play_stop_beep() {
    std::thread::spawn(|| {
        let mut guard = get_player();
        if let Some((_, ref sink)) = *guard {
            sink.clear();
            let tone1 = SineWave::new(440.0)
                .take_duration(Duration::from_millis(100))
                .amplify(1.0);
            sink.append(tone1);
            drop(guard);
            std::thread::sleep(Duration::from_millis(400));
            let mut guard = get_player();
            if let Some((_, ref sink)) = *guard {
                let tone2 = SineWave::new(440.0)
                    .take_duration(Duration::from_millis(100))
                    .amplify(0.5);
                sink.append(tone2);
                sink.sleep_until_end();
            }
        }
    });
}