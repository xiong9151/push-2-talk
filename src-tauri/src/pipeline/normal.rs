// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 自动插入文本
//
// 这是默认的处理模式，保持与原有行为完全兼容
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use anyhow::Result;
use serde_json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode, TranscriptionResultItem};
use crate::config::AppConfig;
use crate::learning::coordinator::start_learning_observation;
use crate::llm_post_processor::LlmPostProcessor;
use crate::tnl::{TnlCandidateDecision, TnlDiagnostics, TnlEngine};

const CANDIDATE_ARBITRATION_TIMEOUT_MS: u64 = 800;

/// 普通模式处理管道
///
/// 职责：
/// 1. 可选的 LLM 后处理（润色、翻译等）
/// 2. 自动插入文本到当前活动窗口
///
/// 设计：无状态，所有依赖通过 process() 参数传入
pub struct NormalPipeline;

impl NormalPipeline {
    /// 创建普通模式管道
    pub fn new() -> Self {
        Self
    }

    /// 处理 ASR 结果
    pub async fn process(
        &self,
        app: &AppHandle,
        post_processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: Vec<String>,
        enable_dictionary_enhancement: bool,
        asr_result: Result<String>,
        asr_time_ms: u64,
        _context: TranscriptionContext, // 普通模式不使用上下文
        target_hwnd: Option<isize>,     // 目标窗口句柄（用于焦点恢复）
        tnl_enabled: bool,
        llm_config: Option<&crate::config::LlmConfig>,
        enable_result_selection: bool,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(PipelineResult, Vec<TranscriptionResultItem>)> {
        // 1. 解包 ASR 结果
        let asr_text = asr_result?;

        // 2. TNL 技术规范化（如果启用）
        let (text, tnl_changed, tnl_diagnostics) = {
            if tnl_enabled {
                let engine = TnlEngine::new(dictionary.clone());
                let tnl_result = engine.normalize(&asr_text);
                if tnl_result.changed {
                    tracing::info!(
                        "NormalPipeline: TNL 规范化: {} → {} (耗时: {}us, 替换: {})",
                        asr_text,
                        tnl_result.text,
                        tnl_result.elapsed_us,
                        tnl_result.applied.len()
                    );
                }
                (tnl_result.text, tnl_result.changed, tnl_result.diagnostics)
            } else {
                (asr_text.clone(), false, None)
            }
        };

        // 3. 可选候选仲裁
        let pre_arbitration_text = text.clone();
        let (text, tnl_diagnostics, candidate_llm_time_ms) = Self::maybe_arbitrate_candidates(
            post_processor.clone(),
            enable_dictionary_enhancement,
            text,
            tnl_diagnostics,
        )
        .await;
        let candidate_changed = text != pre_arbitration_text;

        // 4. 可选 LLM 后处理（可能多预设并行）
        let (items, final_text, original_text, llm_time_ms) = Self::maybe_polish_multi(
            app,
            post_processor,
            enable_post_process,
            &dictionary,
            enable_dictionary_enhancement,
            &text,
            llm_config,
            enable_result_selection,
            cancel_flag,
        )
        .await;
        let combined_llm_time_ms = Self::sum_llm_time(candidate_llm_time_ms, llm_time_ms);

        // 5. 插入前隐藏窗口并主动恢复焦点到目标应用
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 触发学习观察（如果插入成功）
        if let Some(hwnd) = target_hwnd {
            if let Ok((config, _)) = AppConfig::load() {
                if config.learning_config.enabled {
                    start_learning_observation(
                        app.clone(),
                        final_text.clone(),
                        hwnd,
                        config.learning_config,
                    );
                }
            }
        }

        // 7. 返回结果
        let history_original = if original_text.is_some() {
            original_text
        } else if tnl_changed || candidate_changed {
            Some(asr_text)
        } else {
            None
        };

        let mut result = PipelineResult::success(
            final_text,
            history_original,
            None,
            asr_time_ms,
            combined_llm_time_ms,
            TranscriptionMode::Normal,
            false,
        );
        result.tnl_diagnostics = tnl_diagnostics;

        Ok((result, items))
    }

    async fn maybe_arbitrate_candidates(
        processor: Option<LlmPostProcessor>,
        enable_dictionary_enhancement: bool,
        text: String,
        diagnostics: Option<TnlDiagnostics>,
    ) -> (String, Option<TnlDiagnostics>, Option<u64>) {
        let Some(mut diagnostics) = diagnostics else {
            return (text, None, None);
        };

        if !diagnostics.has_pending_llm() {
            return (text, Some(diagnostics), None);
        }

        if !enable_dictionary_enhancement {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedDisabled,
                "dictionary_enhancement_disabled",
                None,
            );
            return (text, Some(diagnostics), None);
        }

        let Some(processor) = processor else {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedNoProcessor,
                "llm_not_configured",
                None,
            );
            return (text, Some(diagnostics), None);
        };

        let fallback_diagnostics = diagnostics.clone();
        let arbitration = tokio::time::timeout(
            Duration::from_millis(CANDIDATE_ARBITRATION_TIMEOUT_MS),
            processor.arbitrate_tnl_candidates(&text, diagnostics),
        )
        .await;

        match arbitration {
            Ok(Ok(result)) => (result.text, Some(result.diagnostics), Some(result.elapsed_ms)),
            Ok(Err(_e)) => {
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedError,
                    "arbitration_error",
                    None,
                );
                (text, Some(diagnostics), None)
            }
            Err(_) => {
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedTimeout,
                    "arbitration_timeout",
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                );
                (text, Some(diagnostics), Some(CANDIDATE_ARBITRATION_TIMEOUT_MS))
            }
        }
    }

    fn sum_llm_time(first: Option<u64>, second: Option<u64>) -> Option<u64> {
        match (first, second) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// LLM 后处理（支持多预设并行）
    async fn maybe_polish_multi(
        app: &AppHandle,
        processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
        text: &str,
        llm_config: Option<&crate::config::LlmConfig>,
        enable_result_selection: bool,
        cancel_flag: Arc<AtomicBool>,
    ) -> (Vec<TranscriptionResultItem>, String, Option<String>, Option<u64>) {
        // 至少包含原文作为第一项
        let mut items: Vec<TranscriptionResultItem> = vec![TranscriptionResultItem {
            id: "original".to_string(),
            label: "原始文本".to_string(),
            text: text.to_string(),
        }];

        if !enable_result_selection && !enable_post_process && !enable_dictionary_enhancement {
            return (items, text.to_string(), None, None);
        }

        if !enable_post_process && enable_dictionary_enhancement && dictionary.is_empty() && !enable_result_selection {
            return (items, text.to_string(), None, None);
        }

        let Some(processor_inner) = processor else {
            return (items, text.to_string(), None, None);
        };

        let presets = llm_config
            .map(|c| c.presets.clone())
            .unwrap_or_default();

        let do_multi = enable_result_selection
            && !presets.is_empty()
            && presets.iter().any(|p| p.selected_for_display);

        if do_multi {
            Self::run_multi_presets(app, processor_inner, text, dictionary, enable_post_process, enable_dictionary_enhancement, &presets, &mut items, cancel_flag).await
        } else {
            Self::run_single_preset(app, processor_inner, text, dictionary, enable_post_process, enable_dictionary_enhancement, &mut items).await
        }
    }

    /// 并行运行多个预设
    async fn run_multi_presets(
        app: &AppHandle,
        processor: LlmPostProcessor,
        text: &str,
        dictionary: &[String],
        enable_post_process: bool,
        enable_dictionary_enhancement: bool,
        presets: &[crate::config::LlmPreset],
        items: &mut Vec<TranscriptionResultItem>,
        cancel_flag: Arc<AtomicBool>,
    ) -> (Vec<TranscriptionResultItem>, String, Option<String>, Option<u64>) {
        let _ = app.emit("post_processing", "polishing");

        let filtered: Vec<_> = presets.iter().filter(|p| p.selected_for_display).collect();

        // 先发射原始文本作为第一个可选项（index=0）
        let _ = app.emit("preset_progress", serde_json::json!({
            "index": 0,
            "name": "原始文本",
            "status": "done",
            "text": text
        }));

        // 先发射所有预设的"处理中"状态（index 从 1 开始）
        for (i, preset) in filtered.iter().enumerate() {
            let _ = app.emit("preset_progress", serde_json::json!({
                "index": i + 1,
                "name": preset.name,
                "status": "processing"
            }));
        }

        let mut handles = Vec::new();
        for (i, preset) in filtered.iter().enumerate() {
            let p = processor.clone();
            let t = text.to_string();
            let d = dictionary.to_vec();
            let pc = (*preset).clone();
            let app_clone = app.clone();
            let cancel = Arc::clone(&cancel_flag);
            handles.push(tokio::spawn(async move {
                let start = Instant::now();
                // 检查是否已被取消
                if cancel.load(Ordering::Relaxed) {
                    let _ = app_clone.emit("preset_progress", serde_json::json!({
                        "index": i + 1,
                        "name": pc.name,
                        "status": "cancelled"
                    }));
                    return (pc, None, start.elapsed().as_millis() as u64);
                }
                let result = p.polish_with_preset(&t, &d, enable_post_process, enable_dictionary_enhancement, &pc).await;
                // 立即发射结果事件
                let _ = app_clone.emit("preset_progress", serde_json::json!({
                    "index": i + 1,
                    "name": pc.name,
                    "status": if result.is_ok() { "done" } else { "error" },
                    "text": result.as_ref().ok().cloned().unwrap_or_default()
                }));
                (pc, Some(result), start.elapsed().as_millis() as u64)
            }));
        }

        let mut first_success: Option<String> = None;
        let mut total_llm_time: u64 = 0;

        for handle in handles {
            match handle.await {
                Ok((preset, Some(Ok(polished)), elapsed)) => {
                    total_llm_time += elapsed;
                    if first_success.is_none() {
                        first_success = Some(polished.clone());
                    }
                    items.push(TranscriptionResultItem {
                        id: format!("preset-{}", items.len()),
                        label: preset.name.clone(),
                        text: polished,
                    });
                }
                Ok((preset, Some(Err(e)), _)) => {
                    tracing::warn!("预设 '{}' 处理失败: {}", preset.name, e);
                }
                Ok((preset, None, _)) => {
                    tracing::info!("预设 '{}' 已被取消", preset.name);
                }
                Err(e) => {
                    tracing::warn!("预设任务 panicked: {}", e);
                }
            }
        }

        let final_text = first_success.unwrap_or_else(|| text.to_string());
        let has_llm = !items.is_empty();
        if !has_llm {
            let _ = app.emit("polishing_failed", "所有润色服务暂时不可用");
        }
        (items.to_vec(), final_text, has_llm.then(|| text.to_string()), Some(total_llm_time))
    }

    /// 单次 LLM 处理（非多结果或只有1个预设时）
    async fn run_single_preset(
        app: &AppHandle,
        processor: LlmPostProcessor,
        text: &str,
        dictionary: &[String],
        enable_post_process: bool,
        enable_dictionary_enhancement: bool,
        items: &mut Vec<TranscriptionResultItem>,
    ) -> (Vec<TranscriptionResultItem>, String, Option<String>, Option<u64>) {
        let _ = app.emit("post_processing", "polishing");
        let llm_start = Instant::now();

        match processor.polish_transcript(text, dictionary, enable_post_process, enable_dictionary_enhancement).await {
            Ok(polished) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                items.push(TranscriptionResultItem {
                    id: "default".to_string(),
                    label: "处理结果".to_string(),
                    text: polished.clone(),
                });
                (items.to_vec(), polished, Some(text.to_string()), Some(elapsed))
            }
            Err(e) => {
                tracing::warn!("LLM 后处理失败，使用原文: {}", e);
                let _ = app.emit("polishing_failed", "润色服务暂时不可用");
                (items.to_vec(), text.to_string(), None, None)
            }
        }
    }
}

impl Default for NormalPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let _pipeline = NormalPipeline::new();
    }
}