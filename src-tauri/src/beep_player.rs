use rodio::{OutputStream, Sink, Source};
use rodio::source::SineWave;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)
const VOLUME: f32 = 0.2;

/// 预初始化的 Sink（OutputStream 通过 leak 保持活跃）
/// Sink 实现了 Send + Sync，安全放入 static
static SHARED_SINK: std::sync::OnceLock<std::sync::Mutex<Sink>> = std::sync::OnceLock::new();

/// 提前初始化音频播放器，消除首次按键延迟
pub fn preinit() {
    if SHARED_SINK.get().is_some() {
        return;
    }
    if let Ok((stream, stream_handle)) = OutputStream::try_default() {
        if let Ok(sink) = Sink::try_new(&stream_handle) {
            // leak OutputStream 使其永久存活
            Box::leak(Box::new(stream));
            let _ = SHARED_SINK.set(std::sync::Mutex::new(sink));
        }
    }
}

/// 获取 sink 并在其上执行操作
fn with_sink<F: FnOnce(&Sink) + Send + 'static>(f: F) {
    if let Some(mutex) = SHARED_SINK.get() {
        std::thread::spawn(move || {
            if let Ok(sink) = mutex.lock() {
                f(&sink);
            }
        });
    } else {
        // fallback: 现场创建
        std::thread::spawn(|| {
            if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&stream_handle) {
                    f(&sink);
                    sink.sleep_until_end();
                }
            }
        });
    }
}

/// 播放提示音（非阻塞）
pub fn play_notification() {
    with_sink(|sink| {
        sink.clear();
        let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
        if let Ok(source) = rodio::Decoder::new(cursor) {
            sink.append(source.amplify(VOLUME));
            sink.sleep_until_end();
        }
    });
}

/// 播放"开始录音"提示音 — 木琴升调：C5(523Hz) → E5(659Hz) → G5(784Hz)
/// 每声 80ms，间隔 80ms，声音清脆有上升感
pub fn play_start_beep() {
    with_sink(|sink| {
        sink.clear();
        let notes = [523.0, 659.0, 784.0]; // C5, E5, G5
        for &freq in &notes {
            let tone = SineWave::new(freq)
                .take_duration(Duration::from_millis(80))
                .amplify(0.3);
            sink.append(tone);
            sink.sleep_until_end();
            std::thread::sleep(Duration::from_millis(80));
        }
    });
}

/// 播放"停止录音"提示音 — 木琴降调：G5(784Hz) → E5(659Hz) → C5(523Hz)
/// 每声 80ms，间隔 80ms，声音清脆有下降感
pub fn play_stop_beep() {
    with_sink(|sink| {
        sink.clear();
        let notes = [784.0, 659.0, 523.0]; // G5, E5, C5
        for &freq in &notes {
            let tone = SineWave::new(freq)
                .take_duration(Duration::from_millis(80))
                .amplify(0.3);
            sink.append(tone);
            sink.sleep_until_end();
            std::thread::sleep(Duration::from_millis(80));
        }
    });
}