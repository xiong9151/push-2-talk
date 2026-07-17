// 音频处理通用工具模块
// 提供音频级别计算、事件发送等共享功能

use anyhow::Result;
use tauri::{AppHandle, Emitter};

/// 音频级别事件 payload
#[derive(Clone, serde::Serialize)]
pub struct AudioLevelPayload {
    pub level: f32,
}

/// 计算音频样本的 RMS 音量级别（0.0 到 1.0）
/// 优化：使用平方根压缩代替对数压缩，保留更大的动态范围
pub fn calculate_audio_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // 计算 RMS (Root Mean Square)
    let sum: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    let rms = (sum / samples.len() as f64).sqrt() as f32;

    // 将 RMS 值映射到 0.0-1.0 范围
    // 语音通常在 0.01-0.3 RMS 范围内，使用 8.0 增益使其更敏感
    let normalized = (rms * 8.0).min(1.0);

    // 使用平方根压缩（比对数更温和，保留更大动态范围）
    // 0.1 → 0.316, 0.5 → 0.707, 1.0 → 1.0
    normalized.sqrt().max(0.0).min(1.0)
}

/// 发送音频级别事件到前端
pub fn emit_audio_level(app: &AppHandle, level: f32) {
    let _ = app.emit("audio_level_update", AudioLevelPayload { level });
}

/// 计算原始 RMS（不带归一化）
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// AGC：自动增益控制（带平滑处理）
/// current_gain: 当前增益状态，用于平滑过渡
///
/// # Known Limitations
/// - FIXME: tanh 后置会限制峰值，即使 gain 正确也可能达不到 TARGET_RMS。
///   缺少闭环反馈：未测量输出 RMS 并与目标对比来调整增益。
///   改进方向：在 tanh 后测量输出 RMS，通过 PI 控制器迭代调整 gain 值。
/// - FIXME: 底噪阈值附近缺少迟滞，gai 在 1.0x 与 5.0x 之间可能突发跳变。
///   当前使用 hysteresis zone 平滑过渡（见下文 NOISE_FLOOR 处理）。
pub fn apply_agc(samples: &mut [f32], current_gain: &mut f32) {
    const TARGET_RMS: f32 = 0.10; // 目标 RMS，平衡小声音放大
    const MAX_GAIN: f32 = 5.0; // 最大增益，平衡微弱声音和抗噪能力
    const MIN_GAIN: f32 = 0.1; // 允许大幅衰减，压住大嗓门
    const NOISE_FLOOR: f32 = 0.003; // 底噪阈值，平衡灵敏度和抗噪能力

    let rms = calculate_rms(samples);

    // 噪声阈值附近迟滞：避免 gain 在 1.0x 与 MAX_GAIN 之间突发跳变
    // - 深底噪 (rms < NOISE_FLOOR * 0.9)：重置 gain 为 1.0，跳过 AGC
    // - 过渡区 (rms < NOISE_FLOOR * 1.1)：保持上次 gain，静默期不衰减
    // - 正常语音：执行完整 AGC 计算
    if rms < NOISE_FLOOR * 0.9 {
        // 深底噪：直接重置 gain 并返回，避免后续突发增益跳变
        *current_gain = 1.0;
        return;
    }
    if rms < NOISE_FLOOR * 1.1 {
        // 过渡迟滞区：保持当前 gain，但不出峰值
        // 这样从底噪进入语音时 gain 不会从 1.0 跳到满值
        // 从语音回到底噪时 gain 不会瞬间归零
        for s in samples.iter_mut() {
            *s = (*s * *current_gain).tanh();
        }
        return;
    }

    // 计算目标增益
    let target_gain = (TARGET_RMS / rms).clamp(MIN_GAIN, MAX_GAIN);

    // 增益平滑：Attack 快（防爆音），Release 慢（防呼吸效应）
    let alpha = if target_gain < *current_gain {
        0.5
    } else {
        0.1
    };
    *current_gain = *current_gain * (1.0 - alpha) + target_gain * alpha;

    for s in samples.iter_mut() {
        *s = (*s * *current_gain).tanh();
    }
}

/// VAD：基于 RMS 阈值判断是否有语音
pub fn is_voice_active(samples: &[f32]) -> bool {
    const THRESHOLD: f32 = 0.003; // 与 NOISE_FLOOR 对齐，平衡灵敏度和抗噪能力
    calculate_rms(samples) > THRESHOLD
}

// ============================================================================
// 无效音频检测
// ============================================================================

/// 无效音频检测阈值
const MIN_AUDIO_DURATION_SAMPLES: usize = 8000; // 0.5秒 @ 16kHz
const MIN_AUDIO_RMS: f32 = 0.02; // 静音阈值（需高于麦克风底噪）

/// 解析 WAV 文件头，找到 PCM 数据在文件中的偏移量（字节位置）
///
/// 不依赖固定的 44 字节偏移量，支持包含 fact 等扩展 Chunk 的 WAV 文件。
fn find_wav_data_offset(wav_data: &[u8]) -> Result<usize> {
    if wav_data.len() < 12 {
        return Err(anyhow::anyhow!("WAV 数据过短"));
    }
    if &wav_data[0..4] != b"RIFF" || &wav_data[8..12] != b"WAVE" {
        return Err(anyhow::anyhow!("无效的 WAV 文件头"));
    }

    // 从 "WAVE" 之后开始遍历 Chunk
    let mut offset = 12;
    while offset + 8 <= wav_data.len() {
        let chunk_id = &wav_data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            wav_data[offset + 4],
            wav_data[offset + 5],
            wav_data[offset + 6],
            wav_data[offset + 7],
        ]) as usize;

        if chunk_id == b"data" {
            // data chunk 内容从 chunk_id(4) + chunk_size(4) 后开始
            return Ok(offset + 8);
        }

        // 跳到下一个 Chunk（每个 Chunk 按偶数字节对齐）
        offset += 8 + chunk_size;
        if chunk_size % 2 != 0 {
            offset += 1;
        }
    }

    Err(anyhow::anyhow!("未在 WAV 文件中找到 data chunk"))
}

/// 验证音频数据是否有效（WAV 格式）
///
/// 检测条件：
/// - 时长 >= 0.5 秒：直接通过
/// - 时长 < 0.5 秒 且 RMS < 0.02（静音）：跳过（用户误触）
/// - 时长 < 0.5 秒 但 RMS >= 0.02（有声音）：继续转写
///
/// 返回 Ok(()) 表示有效，Err 表示无效（包含原因）
pub fn validate_audio(audio_data: &[u8]) -> Result<()> {
    // 检查1：非空
    if audio_data.is_empty() {
        return Err(anyhow::anyhow!("音频数据为空"));
    }

    // 检查2：解析 WAV 头找到实际 data chunk 偏移量（支持扩展 WAV header）
    let data_offset = find_wav_data_offset(audio_data)?;
    let pcm_data = &audio_data[data_offset..];
    let samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();

    // 检查3：时长足够则直接通过
    if samples.len() >= MIN_AUDIO_DURATION_SAMPLES {
        return Ok(());
    }

    // 检查4：时长不足时检查音量（RMS）
    if samples.is_empty() {
        return Err(anyhow::anyhow!("音频数据为空"));
    }

    let sum_squares: f64 = samples.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum();
    let rms = (sum_squares / samples.len() as f64).sqrt() as f32;

    if rms < MIN_AUDIO_RMS {
        tracing::info!(
            "音频过短且静音 ({} 采样点, RMS={:.4})，跳过转写",
            samples.len(),
            rms
        );
        return Err(anyhow::anyhow!("录音过短或无声音，已跳过"));
    }

    // 虽然时长短，但有声音，继续转写
    tracing::info!(
        "音频较短但有声音 ({} 采样点, RMS={:.4})，继续转写",
        samples.len(),
        rms
    );
    Ok(())
}
