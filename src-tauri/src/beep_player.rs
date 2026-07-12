use rodio::{OutputStream, Sink, Source};
use rodio::source::SineWave;
use std::time::Duration;

// 在编译时嵌入音效文件（仅用于通用通知音）
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)
const VOLUME: f32 = 0.2;

/// 播放提示音（非阻塞）
pub fn play_notification() {
    std::thread::spawn(|| {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                let cursor = std::io::Cursor::new(NOTIFICATION_SOUND);
                if let Ok(source) = rodio::Decoder::new(cursor) {
                    sink.append(source.amplify(VOLUME));
                    sink.sleep_until_end();
                }
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
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                // 第一声：50% 音量
                let tone1 = SineWave::new(440.0)
                    .take_duration(Duration::from_millis(100))
                    .amplify(0.5);
                sink.append(tone1);
                sink.sleep_until_end();

                // 间隔 400ms
                std::thread::sleep(Duration::from_millis(400));

                // 第二声：100% 音量
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
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                // 第一声：100% 音量
                let tone1 = SineWave::new(440.0)
                    .take_duration(Duration::from_millis(100))
                    .amplify(1.0);
                sink.append(tone1);
                sink.sleep_until_end();

                // 间隔 400ms
                std::thread::sleep(Duration::from_millis(400));

                // 第二声：50% 音量
                let tone2 = SineWave::new(440.0)
                    .take_duration(Duration::from_millis(100))
                    .amplify(0.5);
                sink.append(tone2);
                sink.sleep_until_end();
            }
        }
    });
}