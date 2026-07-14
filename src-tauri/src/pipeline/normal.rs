// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 返回多结果供选择
//
// Pipeline 不自动插入文本，由调用方决定插入时机
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use anyhow::Result;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode, TranscriptionResultItem};
use crate::llm_post_processor::LlmPostProcessor;
use crate::tnl::{TnlCandidateDecision, TnlDiagnostics, TnlEngine};

const CANDIDATE_ARBITRATION_TIMEOUT_MS: u64 = 800;

/// 普通模式处理管道
///
/// 职责：
/// 1. 可选的 LLM 后处理（润色、翻译等）— 多预设并行
/// 2. 返回多结果列表供用户选择
///
/// 设计：无状态，所有依赖通过 process() 参数传入
pub struct NormalPipeline;

impl NormalPipeline {
    /// 创建普通模式管道
    pub fn new() -> Self {
        Self
    }

    /// 处理 ASR 结果
    ///
    /// # Arguments
    /// * `app` - Tauri 应用句柄（用于发送事件）
    /// * `post_processor` - LLM 后处理器（调用方负责从锁中获取）
    /// * `asr_result` - ASR 转录结果
    /// * `asr_time_ms` - ASR 耗时（毫秒）
    /// * `_context` - 上下文（普通模式不使用）
    /// * `target_hwnd` - 目标窗口句柄（用于焦点恢复）
    /// * `llm_config` - LLM 配置（用于多预设并行处理）
    ///
    /// # Returns
    /// * `Ok((PipelineResult, Vec<TranscriptionResultItem>))` - 处理成功，包含结果列表
    /// * `Err(e)` - 处理失败
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
    ) -> Result<(PipelineResult, Vec<TranscriptionResultItem>)> {
        // 1. 解包 ASR 结果
        let asr_text = asr_result?;
        tracing::info!(
            "NormalPipeline: 收到 ASR 结果: {} (耗时: {}ms)",
            asr_text,
            asr_time_ms
        );

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

        // 注意：历史记录存储 ASR 原文（asr_text），LLM 处理使用 TNL 后文本（text）

        // 3. 可选候选仲裁（绑定词库增强开关，不改变全文润色逻辑）
        let pre_arbitration_text = text.clone();
        let (text, tnl_diagnostics, candidate_llm_time_ms) = Self::maybe_arbitrate_candidates(
            post_processor.clone(),
            enable_dictionary_enhancement,
            text,
            tnl_diagnostics,
        )
        .await;
        let candidate_changed = text != pre_arbitration_text;

        // 4. 多结果并行 LLM 处理
        let (items, final_text, original_text, llm_time_ms) = Self::maybe_polish_multi(
            app,
            post_processor,
            enable_post_process,
            &dictionary,
            enable_dictionary_enhancement,
            &text,
            llm_config,
        )
        .await;
        let combined_llm_time_ms = Self::sum_llm_time(candidate_llm_time_ms, llm_time_ms);

        // 5. 插入前隐藏窗口并主动恢复焦点到目标应用
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 返回结果
        // 历史记录存储 ASR 原文
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
            None, // 普通模式无引用文本
            asr_time_ms,
            combined_llm_time_ms,
            TranscriptionMode::Normal,
            false, // pipeline 不再自动插入文本
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

        tracing::info!(
            "NormalPipeline: 开始 TNL 候选仲裁，候选数: {}",
            diagnostics.pending_llm_count()
        );

        let fallback_diagnostics = diagnostics.clone();
        let arbitration = tokio::time::timeout(
            Duration::from_millis(CANDIDATE_ARBITRATION_TIMEOUT_MS),
            processor.arbitrate_tnl_candidates(&text, diagnostics),
        )
        .await;

        match arbitration {
            Ok(Ok(result)) => {
                tracing::info!(
                    "NormalPipeline: TNL 候选仲裁完成 (耗时: {}ms)",
                    result.elapsed_ms
                );
                (
                    result.text,
                    Some(result.diagnostics),
                    Some(result.elapsed_ms),
                )
            }
            Ok(Err(e)) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁失败，保守跳过: {}", e);
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedError,
                    "arbitration_error",
                    None,
                );
                (text, Some(diagnostics), None)
            }
            Err(_) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁超时，保守跳过");
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedTimeout,
                    "arbitration_timeout",
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                );
                (
                    text,
                    Some(diagnostics),
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                )
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

    /// 多结果 LLM 后处理
    ///
    /// 并行运行所有预设，返回多结果列表
    /// 失败时使用原文
    async fn maybe_polish_multi(
        app: &AppHandle,
        processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
        text: &str,
        llm_config: Option<&crate::config::LlmConfig>,
    ) -> (Vec<TranscriptionResultItem>, String, Option<String>, Option<u64>) {
        // 至少包含原文作为第一项
        let mut items: Vec<TranscriptionResultItem> = vec![TranscriptionResultItem {
            id: "original".to_string(),
            label: "原始文本".to_string(),
            text: text.to_string(),
        }];

        // 如果既没有后处理也没有词库增强，直接返回原文
        if !enable_post_process && !enable_dictionary_enhancement {
            return (items, text.to_string(), None, None);
        }

        // 仅开启词库增强且词库为空：无需调用 LLM
        if !enable_post_process && enable_dictionary_enhancement && dictionary.is_empty() {
            return (items, text.to_string(), None, None);
        }

        let Some(processor_inner) = processor else {
            return (items, text.to_string(), None, None);
        };

        // 获取所有预设
        let presets = llm_config
            .map(|c| {
                if c.presets.is_empty() {
                    vec![]
                } else {
                    c.presets.clone()
                }
            })
            .unwrap_or_default();

        // 如果开启了后处理但没有预设，使用默认单次处理
        if presets.is_empty() {
            tracing::info!("NormalPipeline: 无预设可用，使用单次处理");
            let _ = app.emit("post_processing", "polishing");

            let llm_start = Instant::now();
            match processor_inner
                .polish_transcript(
                    text,
                    dictionary,
                    enable_post_process,
                    enable_dictionary_enhancement,
                )
                .await
            {
                Ok(polished) => {
                    let llm_elapsed = llm_start.elapsed().as_millis() as u64;
                    tracing::info!(
                        "NormalPipeline: LLM 后处理完成: {} (耗时: {}ms)",
                        polished,
                        llm_elapsed
                    );
                    items.push(TranscriptionResultItem {
                        id: "default".to_string(),
                        label: "处理结果".to_string(),
                        text: polished.clone(),
                    });
                    (items, polished, Some(text.to_string()), Some(llm_elapsed))
                }
                Err(e) => {
                    tracing::warn!("NormalPipeline: LLM 后处理失败，使用原文: {}", e);
                    let _ = app.emit("polishing_failed", "润色服务暂时不可用");
                    (items, text.to_string(), None, None)
                }
            }
        } else {
            // 并行处理所有预设
            tracing::info!(
                "NormalPipeline: 开始多结果并行处理，预设数: {}",
                presets.len()
            );

            let _ = app.emit("post_processing", "polishing");

            let mut handles = Vec::new();
            for preset in presets.iter() {
                let processor_clone = processor_inner.clone();
                let text_clone = text.to_string();
                let dict = dictionary.to_vec();
                let enable_pp = enable_post_process;
                let enable_dict = enable_dictionary_enhancement;
                let preset_clone = preset.clone();

                let handle = tokio::spawn(async move {
                    let start = Instant::now();
                    let result = processor_clone
                        .polish_with_preset(
                            &text_clone,
                            &dict,
                            enable_pp,
                            enable_dict,
                            &preset_clone,
                        )
                        .await;
                    let elapsed = start.elapsed().as_millis() as u64;
                    (preset_clone, result, elapsed)
                });
                handles.push(handle);
            }

            // 收集所有结果
            let mut preset_results: Vec<(String, String, u64)> = Vec::new();
            let mut first_success: Option<String> = None;
            let mut total_llm_time: u64 = 0;

            for handle in handles {
                match handle.await {
                    Ok((preset, Ok(polished), elapsed)) => {
                        tracing::info!(
                            "NormalPipeline: 预设 '{}' 处理完成 (耗时: {}ms)",
                            preset.name,
                            elapsed
                        );
                        total_llm_time += elapsed;
                        if first_success.is_none() {
                            first_success = Some(polished.clone());
                        }
                        preset_results.push((preset.name, polished, elapsed));
                    }
                    Ok((preset, Err(e), _)) => {
                        tracing::warn!(
                            "NormalPipeline: 预设 '{}' 处理失败: {}",
                            preset.name,
                            e
                        );
                    }
                    Err(e) => {
                        tracing::warn!("NormalPipeline: 预设任务 panicked: {}", e);
                    }
                }
            }

            // 构建结果列表
            for (name, polished, _elapsed) in &preset_results {
                items.push(TranscriptionResultItem {
                    id: format!("preset-{}", items.len()),
                    label: name.clone(),
                    text: polished.clone(),
                });
            }

            // 确定最终文本（首个成功结果或原文）
            let final_text = first_success.unwrap_or_else(|| text.to_string());
            let has_llm_results = !preset_results.is_empty();
            let llm_time_ms = if has_llm_results {
                Some(total_llm_time)
            } else {
                None
            };
            let original_text = if has_llm_results {
                Some(text.to_string())
            } else {
                None
            };

            if !has_llm_results {
                tracing::warn!("NormalPipeline: 所有预设处理失败，使用原文");
                let _ = app.emit("polishing_failed", "所有润色服务暂时不可用");
            }

            (items, final_text, original_text, llm_time_ms)
        }
    }
}

impl Default for NormalPipeline {
    fn default() -> Self {
        Self::new()
    }
}