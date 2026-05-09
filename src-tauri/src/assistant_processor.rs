// src-tauri/src/assistant_processor.rs
//
// AI 助手处理器
//
// 支持双系统提示词：问答模式和文本处理模式

use anyhow::Result;

use crate::config::{AssistantConfig, SharedLlmConfig};
use crate::openai_client::{ChatOptions, Message, OpenAiClient, OpenAiClientConfig};
use crate::{ConversationTurn, PromptMode};

/// AI 助手处理器
///
/// 根据是否有上下文（选中文本）使用不同的系统提示词
#[derive(Clone)]
pub struct AssistantProcessor {
    client: OpenAiClient,
    /// 问答模式系统提示词（无选中文本时使用）
    qa_system_prompt: String,
    /// 文本处理模式系统提示词（有选中文本时使用）
    text_processing_system_prompt: String,
}

impl AssistantProcessor {
    /// AI 助手模式请求超时（秒）
    ///
    /// 助手模式可能涉及复杂推理，需要比默认 30 秒更长的等待时间
    const ASSISTANT_TIMEOUT_SECS: u64 = 300;

    /// 创建新的 AI 助手处理器实例
    pub fn new(config: AssistantConfig, shared: &SharedLlmConfig) -> Self {
        let resolved = config.resolve_llm(shared);
        let client_config =
            OpenAiClientConfig::new(&resolved.endpoint, &resolved.api_key, &resolved.model)
                .with_timeout_secs(Self::ASSISTANT_TIMEOUT_SECS);
        let client = OpenAiClient::new(client_config);

        Self {
            client,
            qa_system_prompt: config.qa_system_prompt,
            text_processing_system_prompt: config.text_processing_system_prompt,
        }
    }

    /// 处理用户指令（无上下文 - 问答模式）
    ///
    /// # Arguments
    /// * `user_input` - 用户的语音转写文本（问题/指令）
    ///
    /// # Returns
    /// * LLM 的回答
    pub async fn process(&self, user_input: &str) -> Result<String> {
        if user_input.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!("AssistantProcessor: 问答模式处理指令: {}", user_input);

        self.client
            .chat_simple(
                &self.qa_system_prompt,
                user_input,
                ChatOptions::for_smart_command(),
            )
            .await
    }

    /// 带上下文的指令处理（文本处理模式）
    ///
    /// # Arguments
    /// * `user_instruction` - 用户的语音指令
    /// * `selected_text` - 选中的文本
    ///
    /// # Returns
    /// * LLM 处理后的结果
    pub async fn process_with_context(
        &self,
        user_instruction: &str,
        selected_text: &str,
    ) -> Result<String> {
        if user_instruction.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!(
            "AssistantProcessor: 文本处理模式 (指令: {}, 上下文长度: {} 字符)",
            user_instruction,
            selected_text.len()
        );

        // 构建包含上下文的用户消息
        let user_message = format!(
            "【选中的文本】\n{}\n\n【用户指令】\n{}",
            selected_text, user_instruction
        );

        self.client
            .chat_simple(
                &self.text_processing_system_prompt,
                &user_message,
                ChatOptions::for_smart_command(),
            )
            .await
    }

    /// 多轮对话追问处理
    ///
    /// 基于历史对话上下文处理新的用户指令。
    /// system_prompt 由首轮锁定的 PromptMode 决定，追问不改变。
    ///
    /// # Arguments
    /// * `history` - 历史对话轮次
    /// * `new_instruction` - 新的用户语音指令（已经过 TNL 规范化）
    /// * `new_selected_text` - 追问时新选中的文本（可选）
    /// * `prompt_mode` - 首轮锁定的提示词模式
    ///
    /// # Returns
    /// * LLM 的回答
    pub async fn process_followup(
        &self,
        history: &[ConversationTurn],
        new_instruction: &str,
        new_selected_text: Option<&str>,
        prompt_mode: &PromptMode,
    ) -> Result<String> {
        let system_prompt = match prompt_mode {
            PromptMode::QA => &self.qa_system_prompt,
            PromptMode::TextProcessing => &self.text_processing_system_prompt,
        };

        let messages =
            build_followup_messages(system_prompt, history, new_instruction, new_selected_text);

        tracing::info!(
            "AssistantProcessor: 追问模式 (历史轮次: {}, 消息数: {}, 模式: {:?})",
            history.len(),
            messages.len(),
            prompt_mode,
        );

        self.client
            .chat(&messages, ChatOptions::for_smart_command())
            .await
    }
}

// ================== 多轮对话纯函数 ==================

/// 多轮对话最大轮次（LLM 发送时的滑动窗口大小）
const MAX_CONVERSATION_TURNS: usize = 20;

/// 将用户指令和选中文本组合为 user message 内容
///
/// - 有选中文本: `"【选中的文本】\n{selected}\n\n【用户指令】\n{instruction}"`
/// - 无选中文本: 直接返回 instruction
pub(crate) fn format_user_content(instruction: &str, selected_text: Option<&str>) -> String {
    match selected_text {
        Some(text) if !text.is_empty() => {
            format!("【选中的文本】\n{}\n\n【用户指令】\n{}", text, instruction)
        }
        _ => instruction.to_string(),
    }
}

/// 构建多轮对话的 LLM messages 数组
///
/// 结构: `[system, user₁, assistant₁, user₂, assistant₂, ..., userₙ]`
///
/// 当 history 超过 `MAX_CONVERSATION_TURNS` 时，只发送最近 N 轮（滑动窗口）。
pub(crate) fn build_followup_messages(
    system_prompt: &str,
    history: &[ConversationTurn],
    new_instruction: &str,
    new_selected_text: Option<&str>,
) -> Vec<Message> {
    let mut messages = Vec::new();

    // 1. system prompt
    messages.push(Message::system(system_prompt));

    // 2. 历史轮次（滑动窗口截断）
    let window_start = if history.len() > MAX_CONVERSATION_TURNS {
        history.len() - MAX_CONVERSATION_TURNS
    } else {
        0
    };

    for turn in &history[window_start..] {
        messages.push(Message::user(format_user_content(
            &turn.user_instruction,
            turn.selected_text.as_deref(),
        )));
        messages.push(Message::assistant(&turn.assistant_response));
    }

    // 3. 本次追问
    messages.push(Message::user(format_user_content(
        new_instruction,
        new_selected_text,
    )));

    messages
}

/// 将对话历史格式化为 Markdown 用于复制
///
/// 格式:
/// ```text
/// **问**: 用户指令
/// > 选中文本: ...（如有）
///
/// **答**: AI 回复
///
/// ---
///
/// **问**: 追问指令
///
/// **答**: AI 回复
/// ```
pub(crate) fn format_conversation_for_copy(turns: &[ConversationTurn]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for turn in turns {
        let mut section = format!("**问**: {}\n", turn.user_instruction);

        if let Some(ref text) = turn.selected_text {
            if !text.is_empty() {
                section.push_str(&format!("> 选中文本: {}\n", text));
            }
        }

        section.push_str(&format!("\n**答**: {}", turn.assistant_response));

        parts.push(section);
    }

    parts.join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        LlmFeatureConfig, SharedLlmConfig, DEFAULT_ASSISTANT_QA_PROMPT,
        DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT,
    };
    use crate::openai_client::Role;
    use crate::ConversationTurn;

    fn create_test_config() -> AssistantConfig {
        AssistantConfig {
            enabled: true,
            llm: LlmFeatureConfig {
                use_shared: false,
                provider_id: None,
                endpoint: Some("https://api.example.com/v1/chat/completions".to_string()),
                model: Some("test-model".to_string()),
                api_key: Some("test-key".to_string()),
            },
            qa_system_prompt: DEFAULT_ASSISTANT_QA_PROMPT.to_string(),
            text_processing_system_prompt: DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT.to_string(),
        }
    }

    #[test]
    fn test_processor_creation() {
        let config = create_test_config();
        let shared = SharedLlmConfig::default();
        let processor = AssistantProcessor::new(config, &shared);
        assert!(!processor.qa_system_prompt.is_empty());
        assert!(!processor.text_processing_system_prompt.is_empty());
    }

    // === 多轮消息构建测试 ===

    fn make_turn(instruction: &str, selected: Option<&str>, response: &str) -> ConversationTurn {
        ConversationTurn {
            user_instruction: instruction.to_string(),
            selected_text: selected.map(|s| s.to_string()),
            assistant_response: response.to_string(),
            asr_time_ms: 100,
            llm_time_ms: 200,
        }
    }

    #[test]
    fn test_build_followup_messages_basic() {
        // 1 轮历史（QA 模式，无选中文本）+ 追问 1 条纯语音
        let history = vec![make_turn("你好", None, "你好！有什么可以帮你？")];
        let system_prompt = "你是一个助手";

        let messages =
            build_followup_messages(system_prompt, &history, "今天天气怎么样", None);

        // [system, user₁, assistant₁, user₂] = 4 条
        assert_eq!(messages.len(), 4);
        assert!(matches!(messages[0].role, Role::System));
        assert_eq!(messages[0].content, "你是一个助手");
        assert!(matches!(messages[1].role, Role::User));
        assert!(messages[1].content.contains("你好"));
        assert!(matches!(messages[2].role, Role::Assistant));
        assert!(messages[2].content.contains("你好！有什么可以帮你？"));
        assert!(matches!(messages[3].role, Role::User));
        assert!(messages[3].content.contains("今天天气怎么样"));
    }

    #[test]
    fn test_build_followup_messages_with_selected_text() {
        // 1 轮历史 + 追问时带有新选中文本
        let history = vec![make_turn("翻译这段话", Some("Hello world"), "你好世界")];
        let system_prompt = "你是一个文本处理助手";

        let messages = build_followup_messages(
            system_prompt,
            &history,
            "改成正式语气",
            Some("这是新选中的文本"),
        );

        // [system, user₁, assistant₁, user₂] = 4 条
        assert_eq!(messages.len(), 4);
        // 历史 user₁ 应包含选中文本
        assert!(messages[1].content.contains("Hello world"));
        assert!(messages[1].content.contains("翻译这段话"));
        // 新追问 user₂ 应包含新选中文本
        assert!(messages[3].content.contains("这是新选中的文本"));
        assert!(messages[3].content.contains("改成正式语气"));
        assert!(messages[3].content.contains("【选中的文本】"));
        assert!(messages[3].content.contains("【用户指令】"));
    }

    #[test]
    fn test_build_followup_messages_sliding_window() {
        // 25 轮历史（超过 MAX_CONVERSATION_TURNS=20）
        let history: Vec<ConversationTurn> = (0..25)
            .map(|i| make_turn(&format!("问题{}", i), None, &format!("回答{}", i)))
            .collect();
        let system_prompt = "系统提示";

        let messages =
            build_followup_messages(system_prompt, &history, "最新问题", None);

        // 1 (system) + 20*2 (user+assistant) + 1 (new user) = 42
        assert_eq!(messages.len(), 42);
        // 第一条是 system
        assert!(matches!(messages[0].role, Role::System));
        // 应该跳过前 5 轮，从第 5 轮开始
        assert!(messages[1].content.contains("问题5"));
        // 最后一条是新问题
        assert!(messages[41].content.contains("最新问题"));
    }

    #[test]
    fn test_build_followup_messages_text_processing_mode() {
        // TextProcessing 模式的 system prompt 选择验证
        let history = vec![make_turn(
            "润色这段话",
            Some("原始文本"),
            "润色后的文本",
        )];
        let tp_prompt = "你是一个文本处理专家";

        let messages =
            build_followup_messages(tp_prompt, &history, "再简洁一些", None);

        // system prompt 使用传入的 text_processing prompt
        assert_eq!(messages[0].content, "你是一个文本处理专家");
    }

    #[test]
    fn test_format_conversation_for_copy() {
        // 2 轮对话：第 1 轮有选中文本，第 2 轮无
        let turns = vec![
            make_turn("翻译这段话", Some("Hello world"), "你好世界"),
            make_turn("再简洁一些", None, "世界你好"),
        ];

        let output = format_conversation_for_copy(&turns);

        // 第 1 轮：包含问、选中文本、答
        assert!(output.contains("**问**: 翻译这段话"));
        assert!(output.contains("> 选中文本: Hello world"));
        assert!(output.contains("**答**: 你好世界"));
        // 分隔线
        assert!(output.contains("---"));
        // 第 2 轮：包含问、答，无选中文本
        assert!(output.contains("**问**: 再简洁一些"));
        assert!(output.contains("**答**: 世界你好"));
        // 第 2 轮不应包含 "选中文本:" 相关内容（排除第 1 轮的匹配）
        let second_turn_start = output.find("---").unwrap();
        let second_part = &output[second_turn_start..];
        assert!(!second_part.contains("> 选中文本:"));
    }

    #[test]
    fn test_format_conversation_for_copy_single_turn() {
        // 单轮对话不应包含分隔线
        let turns = vec![make_turn("你好", None, "你好！有什么可以帮你？")];

        let output = format_conversation_for_copy(&turns);

        assert!(output.contains("**问**: 你好"));
        assert!(output.contains("**答**: 你好！有什么可以帮你？"));
        assert!(!output.contains("---"));
    }
}
