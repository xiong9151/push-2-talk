// AI 助手模式处理管道
//
// 处理流程：
// 1. 如果有选中文本：上下文 + 语音指令 → ASR → AssistantProcessor (文本处理模式) → 返回结果
// 2. 如果无选中文本：语音指令 → ASR → AssistantProcessor (问答模式) → 返回结果
//
// 不自动插入文本，由调用方通过结果面板（ResultPanelWindow）展示给用户。
// 使用独立的 AssistantProcessor，支持双系统提示词
//
// 注意：多轮对话改造后，此管道由测试直接测试 TNL 集成逻辑使用。
// 生产代码的 LLM 调用已内联到 `handle_assistant_mode()`。

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tnl::TnlEngine;

    #[test]
    fn test_build_tnl_engine_with_empty_dictionary_keeps_phonetic_word() {
        let engine = TnlEngine::new(Vec::new());
        let result = engine.normalize("嗯，我最近学习了他们的那个标准产品 cloud");

        assert!(result.text.contains("cloud"));
        assert!(!result.text.contains("Claude"));
    }

    #[test]
    fn test_build_tnl_engine_with_dictionary_applies_phonetic_replacement() {
        let engine = TnlEngine::new(vec!["Claude".to_string()]);
        let result = engine.normalize("嗯，我最近学习了他们的那个标准产品 cloud");

        assert!(result.text.contains("Claude"));
        assert!(!result.text.contains("cloud"));
    }
}
