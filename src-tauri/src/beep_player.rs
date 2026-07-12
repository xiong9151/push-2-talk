use rodio::{OutputStream, Sink, Source};
use rodio::source::SineWave;
use std::sync::Mutex;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

/// 全局音频 Sink（OutputStream 被泄漏保持活跃，无 Send 约束问题）
static AUDIO_SINK: std::sync::OnceLock<Mutex<Sink>> = std::sync::OnceLock::new();

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

/// 在后台线程中执行播放操作
fn play_in_background<F: FnOnce(&Sink) + Send + 'static>(f: F) {
    if let Some(mutex) = AUDIO_SINK.get() {
        // 预初始化成功：在新线程中获取锁并播放
        std::thread::spawn(move || {
            if let Ok(sink) = mutex.lock() {
                f(&sink);
            }
        });
    } else {
        // 预初始化失败：现场创建播放器
        std::thread::spawn(|| {
            if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&stream_handle) {
                    Box::leak(Box::new(_stream));
                    f(&sink);
                    sink.sleep_until_end();
                }
            }
        });
    }
}

/// 播放提示音（非阻塞）
pub fn play_notification() {
    play_in_background(|sink| {
        sink.clear();
        let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
        if let Ok(source) = rodio::Decoder::new(cursor) {
            sink.append(source.amplify(0.2));
            sink.sleep_until_end();
        }
    });
}

/// 播放"开始录音"提示音
///
/// 第 0 秒：短促的 50% 音量滴声（100ms）
/// 第 0.5 秒：短促的 100% 音量滴声（100ms）
pub fn play_start_beep() {
    play_in_background(|sink| {
        sink.clear();
        let tone1 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(0.5);
        sink.append(tone1);
        sink.sleep_until_end();

        std::thread::sleep(Duration::from_millis(400));

        let tone2 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(1.0);
        sink.append(tone2);
        sink.sleep_until_end();
    });
}

/// 播放"停止录音"提示音
///
/// 第 0 秒：短促的 100% 音量滴声（100ms）
/// 第 0.5 秒：短促的 50% 音量滴声（100ms）
pub fn play_stop_beep() {
    play_in_background(|sink| {
        sink.clear();
        let tone1 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(1.0);
        sink.append(tone1);
        sink.sleep_until_end();

        std::thread::sleep(Duration::from_millis(400));

        let tone2 = SineWave::new(440.0)
            .take_duration(Duration::from_millis(100))
            .amplify(0.5);
        sink.append(tone2);
        sink.sleep_until_end();
    });
}