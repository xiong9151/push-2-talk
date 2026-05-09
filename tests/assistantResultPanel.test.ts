import assert from "node:assert/strict";
import test from "node:test";

// 测试 1: AssistantResultPayload 类型字段完整性
test("AssistantResultPayload 类型导出且包含必要字段", async () => {
  const mod = await import("../src/types/assistant-result");

  // 验证模块导出了类型相关的工具函数（类型本身在编译时验证）
  assert.equal(typeof mod.truncateText, "function");
  assert.equal(typeof mod.formatDuration, "function");
});

// 测试 2: truncateText 工具函数
test("truncateText: 短文本不截断", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  assert.equal(truncateText("hello", 10), "hello");
});

test("truncateText: 超长文本截断并添加省略号", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  const long = "a".repeat(120);
  const result = truncateText(long, 100);
  assert.equal(result.length, 101); // 100 chars + "…"
  assert.ok(result.endsWith("…"));
});

test("truncateText: 恰好等于限制长度不截断", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  const exact = "a".repeat(100);
  assert.equal(truncateText(exact, 100), exact);
});

test("truncateText: 空字符串返回空", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  assert.equal(truncateText("", 100), "");
});

// 测试 3: formatDuration 工具函数
test("formatDuration: 毫秒转秒（带一位小数）", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(1200), "1.2s");
});

test("formatDuration: 整秒显示", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(3000), "3.0s");
});

test("formatDuration: 超过 60 秒显示分+秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(65000), "1m 5s");
});

test("formatDuration: 不足 1 秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(500), "0.5s");
});

test("formatDuration: 0 毫秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(0), "0.0s");
});

test("formatDuration: 恰好 60 秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(60000), "1m 0s");
});

// ==========================================
// Slice 3: ResultPanelWindow 键盘快捷键逻辑
// ==========================================

// getKeyboardAction: 根据按键事件返回应执行的动作
test("getKeyboardAction: Escape → dismiss", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("Escape", false, false), "dismiss");
});

test("getKeyboardAction: Ctrl+C → null (不拦截，保留原生复制)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("c", true, false), null);
});

test("getKeyboardAction: Meta+C → null (不拦截)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("c", false, true), null);
});

test("getKeyboardAction: Enter → null (不再触发粘贴)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("Enter", false, false), null);
});

test("getKeyboardAction: 其他按键 → null", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("a", false, false), null);
  assert.equal(getKeyboardAction("Tab", false, false), null);
  assert.equal(getKeyboardAction("Shift", false, false), null);
});

// ==========================================
// Slice 3 新增: formatConversationForCopy 对话格式化
// ==========================================

test("formatConversationForCopy: 2 轮对话（第 1 轮有选中文本，第 2 轮无）", async () => {
  const { formatConversationForCopy } = await import(
    "../src/types/assistant-result"
  );
  const turns = [
    {
      user_instruction: "翻译这段话",
      selected_text: "Hello world",
      has_selection: true,
      assistant_response: "你好世界",
      asr_time_ms: 500,
      llm_time_ms: 1000,
    },
    {
      user_instruction: "换一种说法",
      selected_text: undefined,
      has_selection: false,
      assistant_response: "世界你好",
      asr_time_ms: 400,
      llm_time_ms: 800,
    },
  ];
  const result = formatConversationForCopy(turns);

  // 包含问答标记
  assert.ok(result.includes("**问**: 翻译这段话"));
  assert.ok(result.includes("**答**: 你好世界"));
  assert.ok(result.includes("**问**: 换一种说法"));
  assert.ok(result.includes("**答**: 世界你好"));

  // 第 1 轮包含选中文本引用
  assert.ok(result.includes("> 选中文本: Hello world"));

  // 第 2 轮不包含选中文本引用
  const secondQuestionIdx = result.indexOf("**问**: 换一种说法");
  const afterSecondQuestion = result.slice(secondQuestionIdx);
  assert.ok(!afterSecondQuestion.includes("> 选中文本:"));

  // 轮次间有分隔线
  assert.ok(result.includes("---"));
});

test("formatConversationForCopy: 单轮对话无分隔线", async () => {
  const { formatConversationForCopy } = await import(
    "../src/types/assistant-result"
  );
  const turns = [
    {
      user_instruction: "今天天气怎样",
      selected_text: undefined,
      has_selection: false,
      assistant_response: "今天晴天",
      asr_time_ms: 300,
      llm_time_ms: 600,
    },
  ];
  const result = formatConversationForCopy(turns);

  assert.ok(result.includes("**问**: 今天天气怎样"));
  assert.ok(result.includes("**答**: 今天晴天"));
  // 单轮无分隔线
  assert.ok(!result.includes("---"));
});

// ==========================================
// formatTimingDisplay: 耗时信息自适应显示
// ==========================================

test("formatTimingDisplay: 语音轮次（asr > 0）显示完整耗时", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(
    formatTimingDisplay(1100, 1200),
    "ASR 1.1s · LLM 1.2s · 总计 2.3s",
  );
});

test("formatTimingDisplay: 文本轮次（asr = 0）只显示 LLM 耗时", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(formatTimingDisplay(0, 1200), "LLM 1.2s");
});

test("formatTimingDisplay: 边界情况（asr = 0, llm = 0）", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(formatTimingDisplay(0, 0), "LLM 0.0s");
});
