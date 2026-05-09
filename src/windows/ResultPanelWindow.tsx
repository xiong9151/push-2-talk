import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Copy,
  X,
  MessageSquare,
  FileText,
  Clock,
  ChevronDown,
  CopyCheck,
  Loader2,
  AlertTriangle,
  SendHorizontal,
} from "lucide-react";
import MarkdownRenderer from "../components/MarkdownRenderer";
import type {
  ConversationTurn,
  ConversationStatePayload,
  TurnPendingPayload,
  TurnCompletePayload,
  TurnErrorPayload,
} from "../types/assistant-result";
import {
  truncateText,
  formatTimingDisplay,
} from "../types/assistant-result";
import type { AppConfig } from "../types";

/** 选中文本摘要最大长度 */
const SELECTED_TEXT_MAX_LENGTH = 100;

/** 复制成功反馈显示时长（毫秒） */
const COPY_FEEDBACK_DURATION_MS = 2000;

/** 轮询间隔（毫秒）— 无结果时向后端拉取 */
const POLL_INTERVAL_MS = 300;

/** 滚动到底部的判定阈值（像素） */
const SCROLL_BOTTOM_THRESHOLD = 30;

/** 开始窗口拖动（透明窗口下 data-tauri-drag-region 不可靠） */
const startDrag = () => {
  getCurrentWindow().startDragging().catch(() => {});
};

export default function ResultPanelWindow() {
  // ==========================================
  // State
  // ==========================================
  const [turns, setTurns] = useState<ConversationTurn[]>([]);
  const [pendingTurn, setPendingTurn] = useState<TurnPendingPayload | null>(
    null,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [theme, setTheme] = useState("light");
  const [copyFeedback, setCopyFeedback] = useState<"latest" | "all" | false>(
    false,
  );
  const [isAtBottom, setIsAtBottom] = useState(true);

  const containerRef = useRef<HTMLDivElement>(null);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isDark = theme === "dark";

  // ==========================================
  // 初始化主题
  // ==========================================
  useEffect(() => {
    invoke<AppConfig>("load_config")
      .then((config) => {
        setTheme(config.theme || "light");
      })
      .catch(console.error);
  }, []);

  // ==========================================
  // 滚动控制
  // ==========================================
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight <
      SCROLL_BOTTOM_THRESHOLD;
    setIsAtBottom(atBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    containerRef.current?.scrollTo({
      top: containerRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, []);

  // 新 turn 或 pendingTurn 到达时自动滚动
  useEffect(() => {
    if (isAtBottom) {
      // 用 requestAnimationFrame 等 DOM 更新完成
      requestAnimationFrame(() => {
        containerRef.current?.scrollTo({
          top: containerRef.current.scrollHeight,
          behavior: "smooth",
        });
      });
    }
  }, [turns.length, pendingTurn, isAtBottom]);

  // ==========================================
  // Push 模式 — 监听 3 个事件 + config_updated
  // ==========================================
  // 注意：React 18 StrictMode 在 dev 模式下会双重挂载组件。
  // 异步 listen() 尚未 resolve 时 cleanup 就执行，导致旧 listener 泄漏。
  // 使用 cancelled flag + 延迟 unsubscribe 确保正确清理。
  useEffect(() => {
    let cancelled = false;
    const cleanups: (() => void)[] = [];

    const setup = async () => {
      const u1 = await listen<TurnCompletePayload>(
        "assistant_turn_complete",
        (event) => {
          if (cancelled) return;
          const { turn, is_followup } = event.payload;
          console.log(
            `[ResultPanel] turn_complete (push), followup=${is_followup}`,
          );

          if (!is_followup) {
            // 首轮：重置为新对话
            setTurns([turn]);
          } else {
            // 追问：追加
            setTurns((prev) => [...prev, turn]);
          }
          setPendingTurn(null);
          setErrorMessage(null);
          setCopyFeedback(false);
        },
      );
      if (cancelled) { u1(); return; }
      cleanups.push(u1);

      const u2 = await listen<TurnPendingPayload>(
        "assistant_turn_pending",
        (event) => {
          if (cancelled) return;
          console.log("[ResultPanel] turn_pending (push)");
          setPendingTurn(event.payload);
          setErrorMessage(null);
        },
      );
      if (cancelled) { u2(); return; }
      cleanups.push(u2);

      const u3 = await listen<TurnErrorPayload>(
        "assistant_turn_error",
        (event) => {
          if (cancelled) return;
          console.log("[ResultPanel] turn_error (push):", event.payload);
          setErrorMessage(event.payload.error_message);
          setPendingTurn(null);
        },
      );
      if (cancelled) { u3(); return; }
      cleanups.push(u3);

      const u4 = await listen<AppConfig>("config_updated", (event) => {
        if (cancelled) return;
        setTheme(event.payload.theme || "light");
      });
      if (cancelled) { u4(); return; }
      cleanups.push(u4);
    };

    setup();

    return () => {
      cancelled = true;
      cleanups.forEach((fn) => fn());
    };
  }, []);

  // ==========================================
  // Poll 模式 — 解决隐藏 WebView 丢失 push 事件
  // ==========================================
  useEffect(() => {
    // 已有 turns 说明 push 正常工作，无需 poll
    if (turns.length > 0) return;

    const fetchState = async () => {
      try {
        const state = await invoke<ConversationStatePayload | null>(
          "get_conversation_state",
        );
        if (state && state.turns.length > 0) {
          console.log("[ResultPanel] 拉取到会话 (poll):", state.session_id);
          setTurns(state.turns);
          setCopyFeedback(false);
        }
      } catch {
        // 静默忽略
      }
    };

    fetchState();
    const interval = setInterval(fetchState, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [turns.length]);

  // ==========================================
  // 操作处理
  // ==========================================
  const handleCopyLatest = useCallback(async () => {
    if (turns.length === 0) return;
    try {
      await invoke("copy_latest_reply");
      setCopyFeedback("latest");
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        setCopyFeedback(false);
      }, COPY_FEEDBACK_DURATION_MS);
    } catch (err) {
      console.error("[ResultPanel] 复制最新回复失败:", err);
    }
  }, [turns.length]);

  const handleCopyAll = useCallback(async () => {
    if (turns.length === 0) return;
    try {
      await invoke("copy_full_conversation");
      setCopyFeedback("all");
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        setCopyFeedback(false);
      }, COPY_FEEDBACK_DURATION_MS);
    } catch (err) {
      console.error("[ResultPanel] 复制全部对话失败:", err);
    }
  }, [turns.length]);

  const handleDismiss = useCallback(async () => {
    try {
      await invoke("dismiss_conversation");
      setTurns([]);
      setPendingTurn(null);
      setErrorMessage(null);
      setCopyFeedback(false);
    } catch (err) {
      console.error("[ResultPanel] 关闭失败:", err);
    }
  }, []);

  // ==========================================
  // 文本追问
  // ==========================================
  const handleTextSend = useCallback(async (text: string) => {
    try {
      await invoke("send_text_question", { text });
    } catch (err) {
      console.error("[ResultPanel] 文本追问失败:", err);
    }
  }, []);

  // ==========================================
  // 键盘快捷键：Esc = 关闭
  // ==========================================
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleDismiss();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleDismiss]);

  // 清理定时器
  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  // ==========================================
  // 无结果时的空状态
  // ==========================================
  if (turns.length === 0 && !pendingTurn) {
    return (
      <div
        onMouseDown={startDrag}
        className={`h-screen flex items-center justify-center select-none cursor-move ${isDark ? "theme-dark" : ""}`}
        style={{
          background: isDark ? "#141413" : "var(--paper)",
          color: isDark ? "#E8E6DC" : "var(--ink)",
          fontFamily: "var(--font-serif)",
          borderRadius: "12px",
        }}
      >
        <p
          style={{
            color: isDark ? "#888" : "var(--stone-dark)",
            fontSize: "14px",
          }}
        >
          等待 AI 助手结果...
        </p>
      </div>
    );
  }

  // ==========================================
  // 渲染：对话流视图
  // ==========================================
  return (
    <div
      className={`h-screen flex flex-col ${isDark ? "theme-dark" : ""}`}
      style={{
        background: "transparent",
        fontFamily: "var(--font-serif)",
      }}
    >
      {/* 外壳：圆角 + 阴影 */}
      <div
        className="flex flex-col flex-1 overflow-hidden"
        style={{
          borderRadius: "12px",
          background: isDark ? "#141413" : "var(--paper)",
          color: isDark ? "#E8E6DC" : "var(--ink)",
          boxShadow: isDark
            ? "0 8px 32px rgba(0,0,0,0.5), 0 2px 8px rgba(0,0,0,0.3)"
            : "0 8px 32px rgba(20,20,19,0.12), 0 2px 8px rgba(20,20,19,0.06)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.1)"
            : "1px solid var(--sand)",
        }}
      >
        {/* ==========================================
            标题栏 (可拖动)
            ========================================== */}
        <div
          onMouseDown={startDrag}
          className="flex items-center justify-between px-4 py-3 select-none cursor-move shrink-0"
          style={{
            background: isDark ? "#1E1E1D" : "var(--sand)",
            borderBottom: isDark
              ? "1px solid rgba(232,230,220,0.08)"
              : "1px solid rgba(232,230,220,0.6)",
            fontFamily: "var(--font-sans)",
          }}
        >
          {/* 左侧：图标 + 标题 */}
          <div className="flex items-center gap-2">
            <span style={{ fontSize: "16px" }}>🤖</span>
            <span
              className="font-semibold text-sm"
              style={{ color: isDark ? "#E8E6DC" : "var(--ink)" }}
            >
              AI 助手
            </span>
            {turns.length > 1 && (
              <span
                className="text-xs px-1.5 py-0.5 rounded-full"
                style={{
                  background: isDark
                    ? "rgba(255,255,255,0.08)"
                    : "rgba(0,0,0,0.06)",
                  color: isDark ? "#999" : "var(--stone-dark)",
                  fontFamily: "var(--font-mono)",
                  fontSize: "11px",
                }}
              >
                {turns.length} 轮
              </span>
            )}
          </div>

          {/* 右侧：关闭按钮 */}
          <button
            onClick={handleDismiss}
            onMouseDown={(e) => e.stopPropagation()}
            className="flex items-center justify-center rounded-md transition-colors"
            style={{
              width: "28px",
              height: "28px",
              color: isDark ? "#888" : "var(--stone-dark)",
              background: "transparent",
              border: "none",
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = isDark
                ? "rgba(255,255,255,0.08)"
                : "rgba(0,0,0,0.06)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
            }}
            title="关闭 (Esc)"
          >
            <X size={16} />
          </button>
        </div>

        {/* ==========================================
            对话流区域 (可滚动)
            ========================================== */}
        <div
          ref={containerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto custom-scrollbar"
          style={{
            background: isDark ? "#1A1A19" : "white",
          }}
        >
          {turns.map((turn, i) => (
            <div key={`turn-${i}`}>
              {/* 轮次分隔线 */}
              {i > 0 && <TurnDivider isDark={isDark} index={i} />}

              {/* 用户气泡 */}
              <UserBubble
                instruction={turn.user_instruction}
                selectedText={turn.selected_text}
                hasSelection={turn.has_selection}
                isDark={isDark}
              />

              {/* AI 回复 */}
              <AssistantBubble
                response={turn.assistant_response}
                asrTimeMs={turn.asr_time_ms}
                llmTimeMs={turn.llm_time_ms}
                isDark={isDark}
              />
            </div>
          ))}

          {/* 追问 pending 状态 */}
          {pendingTurn && (
            <div>
              {turns.length > 0 && (
                <TurnDivider isDark={isDark} index={turns.length} />
              )}
              <UserBubble
                instruction={pendingTurn.user_instruction}
                selectedText={pendingTurn.selected_text}
                hasSelection={pendingTurn.has_selection}
                isDark={isDark}
              />
              <LoadingBubble isDark={isDark} />
            </div>
          )}

          {/* 错误气泡 */}
          {errorMessage && <ErrorBubble message={errorMessage} isDark={isDark} />}
        </div>

        {/* ==========================================
            "查看最新回复" 浮标
            ========================================== */}
        {!isAtBottom && (turns.length > 1 || pendingTurn) && (
          <div
            style={{
              position: "relative",
            }}
          >
            <button
              onClick={scrollToBottom}
              className="flex items-center gap-1 px-3 py-1.5 rounded-full text-xs transition-all"
              style={{
                position: "absolute",
                bottom: "8px",
                right: "16px",
                background: isDark
                  ? "rgba(255,255,255,0.12)"
                  : "rgba(0,0,0,0.06)",
                color: isDark ? "#ccc" : "var(--ink)",
                border: isDark
                  ? "1px solid rgba(255,255,255,0.15)"
                  : "1px solid rgba(0,0,0,0.08)",
                cursor: "pointer",
                backdropFilter: "blur(8px)",
                zIndex: 10,
                fontFamily: "var(--font-sans)",
              }}
            >
              <ChevronDown size={14} />
              <span>查看最新回复</span>
            </button>
          </div>
        )}

        {/* ==========================================
            文本输入栏
            ========================================== */}
        <TextInputBar
          isDark={isDark}
          isProcessing={!!pendingTurn}
          onSend={handleTextSend}
        />

        {/* ==========================================
            操作栏
            ========================================== */}
        <div
          className="flex items-center justify-end gap-2 px-4 py-3 shrink-0"
          style={{
            background: isDark ? "#1E1E1D" : "var(--sand)",
            borderTop: isDark
              ? "1px solid rgba(232,230,220,0.08)"
              : "1px solid rgba(232,230,220,0.6)",
            fontFamily: "var(--font-sans)",
          }}
        >
          {/* 关闭按钮（次要） */}
          <button
            onClick={handleDismiss}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-all"
            style={{
              border: isDark ? "1px solid #333" : "1px solid var(--sand)",
              background: isDark ? "rgba(255,255,255,0.05)" : "white",
              color: isDark ? "#ccc" : "var(--ink)",
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = isDark
                ? "rgba(255,255,255,0.1)"
                : "rgba(0,0,0,0.03)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = isDark
                ? "rgba(255,255,255,0.05)"
                : "white";
            }}
          >
            <X size={14} />
            <span>关闭</span>
          </button>

          {/* 复制全部（多轮时显示） */}
          {turns.length > 1 && (
            <button
              onClick={handleCopyAll}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-all"
              style={{
                border: isDark ? "1px solid #333" : "1px solid var(--sand)",
                background: isDark ? "rgba(255,255,255,0.05)" : "white",
                color: isDark ? "#ccc" : "var(--ink)",
                cursor: "pointer",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = isDark
                  ? "rgba(255,255,255,0.1)"
                  : "rgba(0,0,0,0.03)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = isDark
                  ? "rgba(255,255,255,0.05)"
                  : "white";
              }}
            >
              {copyFeedback === "all" ? (
                <CopyCheck size={14} />
              ) : (
                <Copy size={14} />
              )}
              <span>{copyFeedback === "all" ? "已复制全部" : "复制全部"}</span>
            </button>
          )}

          {/* 复制最新回复（主要） */}
          <button
            onClick={handleCopyLatest}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-sm font-medium transition-all"
            style={{
              background: "var(--crail)",
              color: "white",
              border: "none",
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.opacity = "0.9";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.opacity = "1";
            }}
          >
            {copyFeedback === "latest" ? (
              <CopyCheck size={14} />
            ) : (
              <Copy size={14} />
            )}
            <span>
              {copyFeedback === "latest"
                ? "已复制"
                : turns.length > 1
                  ? "复制最新回复"
                  : "复制"}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}

// ==========================================
// 子组件
// ==========================================

/** 轮次分隔线 */
function TurnDivider({ isDark, index }: { isDark: boolean; index: number }) {
  return (
    <div
      className="flex items-center gap-3 px-4 py-2"
      style={{
        color: isDark ? "#666" : "var(--stone-dark)",
        fontSize: "11px",
        fontFamily: "var(--font-sans)",
      }}
    >
      <div
        className="flex-1"
        style={{
          height: "1px",
          background: isDark
            ? "rgba(232,230,220,0.08)"
            : "rgba(0,0,0,0.06)",
        }}
      />
      <span>追问 #{index}</span>
      <div
        className="flex-1"
        style={{
          height: "1px",
          background: isDark
            ? "rgba(232,230,220,0.08)"
            : "rgba(0,0,0,0.06)",
        }}
      />
    </div>
  );
}

/** 用户气泡 */
function UserBubble({
  instruction,
  selectedText,
  hasSelection,
  isDark,
}: {
  instruction: string;
  selectedText?: string;
  hasSelection: boolean;
  isDark: boolean;
}) {
  return (
    <div className="px-4 py-3" style={{ fontSize: "13px" }}>
      {/* 语音指令 */}
      <div className="flex items-start gap-2">
        <MessageSquare
          size={14}
          className="shrink-0 mt-0.5"
          style={{ color: isDark ? "#888" : "var(--stone-dark)" }}
        />
        <span
          style={{
            color: isDark ? "#ccc" : "var(--ink)",
            lineHeight: 1.5,
          }}
        >
          {instruction}
        </span>
      </div>

      {/* 选中文本摘要 */}
      {hasSelection && selectedText && (
        <div className="flex items-start gap-2 mt-2">
          <FileText
            size={14}
            className="shrink-0 mt-0.5"
            style={{ color: isDark ? "#888" : "var(--stone-dark)" }}
          />
          <span
            style={{
              color: isDark ? "#999" : "var(--stone-dark)",
              lineHeight: 1.5,
              fontSize: "12px",
            }}
          >
            原文: {truncateText(selectedText, SELECTED_TEXT_MAX_LENGTH)}
          </span>
        </div>
      )}
    </div>
  );
}

/** AI 回复气泡 */
function AssistantBubble({
  response,
  asrTimeMs,
  llmTimeMs,
  isDark,
}: {
  response: string;
  asrTimeMs: number;
  llmTimeMs: number;
  isDark: boolean;
}) {
  return (
    <div className="px-4 pb-3">
      {/* Markdown 回复 */}
      <div
        className="rounded-lg px-3 py-2"
        style={{
          background: isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.06)"
            : "1px solid rgba(0,0,0,0.04)",
        }}
      >
        <MarkdownRenderer content={response} darkMode={isDark} />
      </div>

      {/* 耗时信息 */}
      <div
        className="flex items-center gap-2 mt-1.5 px-1"
        style={{
          fontSize: "11px",
          color: isDark ? "#555" : "var(--stone-dark)",
          fontFamily: "var(--font-mono)",
        }}
      >
        <Clock size={10} />
        <span>{formatTimingDisplay(asrTimeMs, llmTimeMs)}</span>
      </div>
    </div>
  );
}

/** 加载中气泡 */
function LoadingBubble({ isDark }: { isDark: boolean }) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-center gap-2 rounded-lg px-3 py-3"
        style={{
          background: isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.06)"
            : "1px solid rgba(0,0,0,0.04)",
          color: isDark ? "#888" : "var(--stone-dark)",
          fontSize: "13px",
        }}
      >
        <Loader2 size={14} className="animate-spin" />
        <span>AI 思考中...</span>
      </div>
    </div>
  );
}

/** 错误气泡 */
function ErrorBubble({
  message,
  isDark,
}: {
  message: string;
  isDark: boolean;
}) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-start gap-2 rounded-lg px-3 py-2"
        style={{
          background: isDark
            ? "rgba(239,68,68,0.08)"
            : "rgba(239,68,68,0.05)",
          border: "1px solid rgba(239,68,68,0.2)",
          color: isDark ? "#f87171" : "#dc2626",
          fontSize: "13px",
        }}
      >
        <AlertTriangle size={14} className="shrink-0 mt-0.5" />
        <span style={{ lineHeight: 1.5 }}>{message}</span>
      </div>
    </div>
  );
}

/** 文本追问输入栏 */
function TextInputBar({
  isDark,
  isProcessing,
  onSend,
}: {
  isDark: boolean;
  isProcessing: boolean;
  onSend: (text: string) => void;
}) {
  const [inputText, setInputText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const canSend = inputText.trim().length > 0 && !isProcessing;

  const handleSend = useCallback(() => {
    const trimmed = inputText.trim();
    if (!trimmed || isProcessing) return;
    onSend(trimmed);
    setInputText("");
    // 重置 textarea 高度
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [inputText, isProcessing, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter 发送（非空时），Shift+Enter 换行
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
      // Esc 不做任何拦截，让事件冒泡到 window listener 处理关闭
    },
    [handleSend],
  );

  // textarea 自动调整高度
  const handleInput = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setInputText(e.target.value);
      const el = e.target;
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 72)}px`;
    },
    [],
  );

  return (
    <div
      className="flex items-end gap-2 px-4 py-2 shrink-0"
      style={{
        borderTop: isDark
          ? "1px solid rgba(232,230,220,0.08)"
          : "1px solid rgba(232,230,220,0.6)",
        fontFamily: "var(--font-sans)",
      }}
    >
      <textarea
        ref={textareaRef}
        value={inputText}
        onChange={handleInput}
        onKeyDown={handleKeyDown}
        disabled={isProcessing}
        placeholder="输入追问..."
        rows={1}
        className="flex-1 text-sm rounded-lg px-3 py-2 outline-none transition-colors"
        style={{
          resize: "none",
          maxHeight: "72px",
          overflowY: "auto",
          background: isDark ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.03)",
          border: isDark
            ? "1px solid rgba(255,255,255,0.1)"
            : "1px solid rgba(0,0,0,0.08)",
          color: isDark ? "#E8E6DC" : "var(--ink)",
          opacity: isProcessing ? 0.5 : 1,
        }}
      />
      <button
        onClick={handleSend}
        disabled={!canSend}
        className="flex items-center justify-center rounded-lg transition-all shrink-0"
        style={{
          width: "34px",
          height: "34px",
          background: canSend ? "var(--crail)" : isDark ? "#333" : "#ddd",
          color: canSend ? "white" : isDark ? "#666" : "#999",
          border: "none",
          cursor: canSend ? "pointer" : "not-allowed",
          opacity: canSend ? 1 : 0.6,
        }}
      >
        <SendHorizontal size={16} />
      </button>
    </div>
  );
}
