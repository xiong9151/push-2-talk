use rodio::{OutputStream, Sink, Source};
use rodio::source::SineWave;
use std::sync::Mutex;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

/// 预初始化的 Sink（OutputStream 被泄漏以保持音频设备活动）
static AUDIO_SINK: once_cell::sync::OnceCell<Mutex<Sink>> = once_cell::sync::OnceCell::new();

/// 提前初始化音频播放器（在应用启动时调用，消除首次按键的延迟）
pub fn preinit() {
    if AUDIO_SINK.get().is_some() {
        return;
    }
    if let Ok((stream, stream_handle)) = OutputStream::try_default() {
        if let Ok(sink) = Sink::try_new(&stream_handle) {
            // 泄漏 OutputStream 使其永久活动，audio 数据持续播放
            Box::leak(Box::new(stream));
            let _ = AUDIO_SINK.set(Mutex::new(sink));
        }
    }
}

/// 播放提示音（非阻塞）
pub fn play_notification() {
    std::thread::spawn(|| {
        let guard = AUDIO_SINK.get()?;
        let sink = guard.lock().ok()?;
        let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
        let source = rodio::Decoder::new(cursor).ok()?;
        sink.clear();
        sink.append(source.amplify(0.2));
        sink.sleep_until_end();
        Some(())
    });
}

/// 播放"开始录音"提示音
///
/// 第 0 秒：短促的 50% 音量滴声（100ms）
/// 第 0.5 秒：短促的 100% 音量滴声（100ms）
pub fn play_start_beep() {
    std::thread::spawn(|| {
        let guard = AUDIO_SINK.get()?;
        let sink = guard.lock().ok()?;
        sink.clear();
        let tone1 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(0.5);
        sink.append(tone1);
        // 释放锁，让音频回调能处理数据
        drop(sink);
        drop(guard);
        // 等待 400ms，使两声之间间隔 500ms（第一声 100ms + 400ms 静音）
        std::thread::sleep(Duration::from_millis(400));
        let guard = AUDIO_SINK.get()?;
        let sink = guard.lock().ok()?;
        let tone2 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(1.0);
        sink.append(tone2);
        sink.sleep_until_end();
        Some(())
    });
}

/// 播放"停止录音"提示音
///
/// 第 0 秒：短促的 100% 音量滴声（100ms）
/// 第 0.5 秒：短促的 50% 音量滴声（100ms）
pub fn play_stop_beep() {
    std::thread::spawn(|| {
        let guard = AUDIO_SINK.get()?;
        let sink = guard.lock().ok()?;
        sink.clear();
        let tone1 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(1.0);
        sink.append(tone1);
        drop(sink);
        drop(guard);
        std::thread::sleep(Duration::from_millis(400));
        let guard = AUDIO_SINK.get()?;
        let sink = guard.lock().ok()?;
        let tone2 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(0.5);
        sink.append(tone2);
        sink.sleep_until_end();
        Some(())
    });
}