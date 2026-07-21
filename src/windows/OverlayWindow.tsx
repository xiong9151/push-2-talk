import { AppConfig } from "../types";
import { useState, useEffect, useRef, useCallback } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// 音频级别事件 payload 类型
interface AudioLevelPayload {
  level: number;
}

// 转录结果项
interface TranscriptionResultItem {
  id: string;
  label: string;
  text: string;
}

// 预设进度项
interface PresetProgress {
  index: number;
  name: string;
  status: "processing" | "done" | "error" | "pending" | "cancelled";
  text?: string | null;
}

// 状态类型
type OverlayStatus = "recording" | "transcribing" | "results";

// 静音阈值常量（与后端 NOISE_FLOOR 对齐）
const SILENCE_THRESHOLD = 0.005;

// Hook 返回类型
interface AudioVisualizationState {
  level: number;
  time: number;
}

// ==========================================
// 核心：高性能音频可视化 Hook
// ==========================================
const useSmoothAudioLevel = (isRecording: boolean): AudioVisualizationState => {
  const [state, setState] = useState<AudioVisualizationState>({ level: 0, time: 0 });
  const targetRef = useRef(0);
  const currentRef = useRef(0);
  const timeRef = useRef(0);
  const animationRef = useRef<number>(0);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    const setup = async () => {
      const u = await listen<AudioLevelPayload>("audio_level_update", (event) => {
        targetRef.current = Math.min(Math.pow(event.payload.level, 0.45), 1.0);
      });
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    };
    setup();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (!isRecording) {
      cancelAnimationFrame(animationRef.current);
      currentRef.current = 0;
      targetRef.current = 0;
      timeRef.current = 0;
      setState({ level: 0, time: 0 });
      return;
    }

    const animate = () => {
      const target = targetRef.current;
      const current = currentRef.current;
      const speed = target > current ? 0.5 : 0.06;
      currentRef.current += (target - current) * speed;

      if (currentRef.current < SILENCE_THRESHOLD) {
        currentRef.current = 0;
      }

      timeRef.current = (timeRef.current + 0.08) % (Math.PI * 100);

      setState({
        level: currentRef.current,
        time: timeRef.current
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationRef.current);
  }, [isRecording]);

  return state;
};

// ==========================================
// 组件部分
// ==========================================

function WaveBar({ height }: { height: number }) {
  return (
    <div
      className="wave-bar"
      style={{ height: `${height}px` }}
    />
  );
}

function WaveformBars({ level, time }: { level: number; time: number }) {
  const minHeight = 3;
  const maxHeight = 28;
  const baseScales = [0.3, 0.45, 0.65, 0.85, 1.0, 0.85, 0.65, 0.45, 0.3];

  return (
    <div className="wave-container">
      {baseScales.map((baseScale, i) => {
        if (level === 0) {
          return <WaveBar key={i} height={minHeight} />;
        }
        const distanceFromCenter = Math.abs(i - 4);
        const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;
        const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;
        const dynamicScale = baseScale + flowWave * level + turbulence;
        const height = minHeight + level * dynamicScale * (maxHeight - minHeight);
        return <WaveBar key={i} height={Math.max(minHeight, Math.min(maxHeight, height))} />;
      })}
    </div>
  );
}

function LoadingIndicator() {
  return (
    <div className="loading-container">
      <div className="dots-container">
        {[...Array(9)].map((_, i) => (
          <div key={i} className="dot" style={{ animationDelay: `${i * 0.1}s` }} />
        ))}
      </div>
      <div className="spinner-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="4" />
          <line x1="12" y1="2" x2="12" y2="6" />
          <line x1="12" y1="18" x2="12" y2="22" />
          <line x1="2" y1="12" x2="6" y2="12" />
          <line x1="18" y1="12" x2="22" y2="12" />
          <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
          <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
          <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" />
          <line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
        </svg>
      </div>
    </div>
  );
}

function LockedControls({
  onFinish,
  onCancel,
  level,
  time,
  disabled
}: {
  onFinish: () => void;
  onCancel: () => void;
  level: number;
  time: number;
  disabled: boolean;
}) {
  const minHeight = 3;
  const maxHeight = 26;
  const baseScales = [0.35, 0.55, 0.8, 1.0, 0.8, 0.55, 0.35];

  return (
    <div className="locked-controls">
      <button
        onClick={onCancel}
        disabled={disabled}
        className={`locked-btn locked-btn-cancel ${disabled ? 'opacity-50' : ''}`}
        title="取消 (Esc)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>

      <div className="locked-wave-mini">
        {baseScales.map((baseScale, i) => {
          if (level === 0) {
            return (
              <div key={i} className="wave-bar-mini" style={{ height: `${minHeight}px` }} />
            );
          }
          const distanceFromCenter = Math.abs(i - 3);
          const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;
          const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;
          const dynamicScale = baseScale + flowWave * level + turbulence;
          const height = minHeight + level * dynamicScale * (maxHeight - minHeight);

          return (
            <div
              key={i}
              className="wave-bar-mini"
              style={{ height: `${Math.max(minHeight, Math.min(maxHeight, height))}px` }}
            />
          );
        })}
      </div>

      <button
        onClick={onFinish}
        disabled={disabled}
        className={`locked-btn locked-btn-finish ${disabled ? 'opacity-50' : ''}`}
        title="发送 (Enter)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      </button>
    </div>
  );
}

// 结果列表组件
function ResultList({
  items,
  selectedIndex,
  onSelect,
  onConfirm
}: {
  items: TranscriptionResultItem[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onConfirm: (item: TranscriptionResultItem) => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  // Auto-scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const selectedEl = listRef.current.querySelector('.result-item-selected');
    if (selectedEl) {
      selectedEl.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  return (
    <div className="result-list" ref={listRef}>
      <div className="result-list-header">选择处理结果</div>
      {items.map((item, index) => (
        <div
          key={item.id}
          className={`result-item ${index === selectedIndex ? 'result-item-selected' : ''}`}
          onClick={() => onConfirm(item)}
          onMouseEnter={() => onSelect(index)}
        >
          <div className="result-item-label">{item.label}</div>
          <div className="result-item-text">{item.text}</div>
        </div>
      ))}
      <div className="result-list-hint">
        Tab 选择 · Enter 确认 · Esc 取消
      </div>
    </div>
  );
}

// 预设进度列表组件（复用 ResultList 样式）
function PresetProgressList({
  items,
  selectedIndex,
  onSelect,
  onConfirm,
  disabled
}: {
  items: PresetProgress[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onConfirm: (index: number) => void;
  disabled: boolean;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!listRef.current) return;
    const selectedEl = listRef.current.querySelector('.result-item-selected');
    if (selectedEl) {
      selectedEl.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  return (
    <div className="result-list" ref={listRef}>
      <div className="result-list-header">选择处理结果</div>
      {items.map((item, index) => (
        <div
          key={index}
          className={`result-item ${index === selectedIndex ? 'result-item-selected' : ''} ${
            item.status === "processing" ? 'result-item-loading' : ''
          }`}
          onClick={() => {
            if (item.status === "done" && !disabled) {
              onConfirm(index);
            }
          }}
          onMouseEnter={() => {
            if (item.status === "done") onSelect(index);
          }}
          style={{ opacity: item.status === "pending" ? 0.4 : 1 }}
        >
          <div className="result-item-label">
            {item.name || `预设 ${index + 1}`}
            {item.status === "processing" && <span className="result-item-spinner" />}
          </div>
          <div className="result-item-text">
            {item.status === "done" && item.text ? item.text : ''}
            {item.status === "processing" && <span className="result-item-processing-text">正在处理...</span>}
            {item.status === "pending" && <span className="result-item-pending-text">等待中...</span>}
            {item.status === "error" && <span className="result-item-error-text">处理失败</span>}
            {item.status === "cancelled" && <span className="result-item-error-text">已取消</span>}
          </div>
        </div>
      ))}
      <div className="result-list-hint">
        Tab 选择 · Enter 确认 · Esc 取消
      </div>
    </div>
  );
}

// 主悬浮窗组件
export default function OverlayWindow() {
  const [status, setStatus] = useState<OverlayStatus>("recording");
  const statusRef = useRef(status);
  useEffect(() => { statusRef.current = status; }, [status]);
  const [isLocked, setIsLocked] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const submitDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [theme, setTheme] = useState("light");
  const [resultItems, setResultItems] = useState<TranscriptionResultItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [liveTranscript, setLiveTranscript] = useState("");
  const [enableLiveTranscript, setEnableLiveTranscript] = useState(false);
  const [presetResults, setPresetResults] = useState<PresetProgress[]>([]);
  const hasSelectedResultRef = useRef(false); // 用户是否已选择了一个结果（用 ref 确保闭包可见）
  const hasEnteredResultsRef = useRef(false); // 是否已进入 results 模式（防止重复弹出）

  const { level: audioLevel, time: animationTime } = useSmoothAudioLevel(status === "recording");

  // 选择结果并插入
  const confirmResult = useCallback(async (item: TranscriptionResultItem) => {
    setIsSubmitting(true);
    try {
      await invoke("select_transcription_result", { text: item.text });
      // 重置状态
      setStatus("recording");
      setResultItems([]);
      setSelectedIndex(0);
    } catch (e) {
      console.error("选择结果失败:", e);
    }
    setIsSubmitting(false);
  }, []);

  // 选择预设结果（渐进式结果）
  const handleSelectPresetResult = async (index: number) => {
    if (hasSelectedResultRef.current) return; // 防重复
    hasSelectedResultRef.current = true;
    setIsSubmitting(true);
    try {
      // 取消其他未完成的任务
      await invoke("cancel_pending_presets");
      // 选中该结果
      if (presetResults[index].text) {
        await invoke("select_transcription_result", { text: presetResults[index].text });
      }
      // 重置状态
      setStatus("recording");
      setPresetResults([]);
      hasEnteredResultsRef.current = false;
    } catch (e) {
      console.error("选择预设结果失败:", e);
      hasSelectedResultRef.current = false;
    }
    setIsSubmitting(false);
  };

  // Auto-focus window when entering results mode (for keyboard events)
  useEffect(() => {
    if (status === "results") {
      // Small delay to let the overlay render before focusing
      const timer = setTimeout(() => {
        window.focus();
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [status]);

  // 键盘导航
  useEffect(() => {
    if (status !== "results") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Tab") {
        e.preventDefault();
        e.stopPropagation();
        if (e.shiftKey) {
          setSelectedIndex(prev => Math.max(prev - 1, 0));
        } else {
          setSelectedIndex(prev => Math.min(prev + 1, resultItems.length - 1));
        }
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        if (resultItems[selectedIndex]) {
          confirmResult(resultItems[selectedIndex]);
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        // 选择原文并插入
        if (resultItems[0]) {
          confirmResult(resultItems[0]);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [status, resultItems, selectedIndex, confirmResult]);

  useEffect(() => {
    invoke<AppConfig>("load_config").then(config => {
      setTheme(config.theme || "light");
      setEnableLiveTranscript(config.enable_live_transcript || false);
    }).catch(console.error);
  }, []);

  useEffect(() => {
    const unlistenFns: UnlistenFn[] = [];
    let cancelled = false;

    const setup = async () => {
      const registerListener = async (
        event: string,
        handler: (event: any) => void,
      ): Promise<boolean> => {
        const unlisten = await listen(event, handler);
        if (cancelled) {
          unlisten();
          return false;
        }
        unlistenFns.push(unlisten);
        return true;
      };

      if (!(await registerListener("config_updated", (event) => {
        const config = event.payload as AppConfig;
        console.log("[OverlayWindow] 收到 config_updated 事件, theme=", config.theme);
        setTheme(config.theme || "light");
        setEnableLiveTranscript(config.enable_live_transcript || false);
      }))) return;

      if (!(await registerListener("recording_started", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
        setResultItems([]);
        setSelectedIndex(0);
        setLiveTranscript("");
        setPresetResults([]);
        hasSelectedResultRef.current = false;
        hasEnteredResultsRef.current = false;
      }))) return;

      if (!(await registerListener("recording_locked", () => {
        console.log("进入松手模式");
        setIsLocked(true);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("recording_stopped", () => {
        setStatus("transcribing");
      }))) return;

      if (!(await registerListener("transcribing", () => {
        setStatus("transcribing");
      }))) return;

      if (!(await registerListener("transcription_complete", () => {
        // 如果已经通过 preset_progress 显示了结果，不覆盖
        // 如果用户已选择了一个结果，不重置
        // 否则重置
        if (!hasEnteredResultsRef.current && !hasSelectedResultRef.current) {
          setStatus("recording");
          setIsLocked(false);
          setIsSubmitting(false);
        }
      }))) return;

      if (!(await registerListener("error", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("transcription_cancelled", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("transcription_results", (event) => {
        // 如果已经通过 preset_progress 显示了结果，不覆盖
        if (hasEnteredResultsRef.current) return;
        const items = event.payload as TranscriptionResultItem[];
        console.log("[OverlayWindow] 收到转录结果列表:", items);
        setResultItems(items);
        setSelectedIndex(0);
        setStatus("results");
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("live_transcript", (event) => {
        const text = event.payload as string;
        console.log("[OverlayWindow] 实时转录:", text);
        setLiveTranscript(text);
      }))) return;

      if (!(await registerListener("preset_progress", (event) => {
        const payload = event.payload as PresetProgress;
        // 用户已选择结果，忽略后续事件
        if (hasSelectedResultRef.current) return;
        setPresetResults(prev => {
          const results = [...prev];
          const idx = payload.index;
          // 确保数组长度足够
          while (results.length <= idx) {
            results.push({ index: results.length, name: "", status: "pending", text: null });
          }
          results[idx] = {
            index: payload.index,
            name: payload.name,
            status: payload.status,
            text: payload.text || null,
          };
          return results;
        });
        // 进入结果模式（仅第一次）
        if (!hasEnteredResultsRef.current) {
          hasEnteredResultsRef.current = true;
          setStatus("results");
        }
      }))) return;
    };

    setup();

    return () => {
      cancelled = true;
      unlistenFns.forEach(fn => fn());
    };
  }, []);

  useEffect(() => {
    if (status === "transcribing") {
      const timeout = setTimeout(async () => {
        console.warn("转写超时 15 秒，强制隐藏悬浮窗");
        try {
          await invoke("hide_overlay");
          setStatus("recording");
          setIsLocked(false);
          setIsSubmitting(false);
        } catch (e) {
          console.error("强制隐藏悬浮窗失败:", e);
        }
      }, 15000);
      return () => clearTimeout(timeout);
    }
  }, [status]);

  useEffect(() => {
    if (isLocked && !isSubmitting) {
      const timeout = setTimeout(async () => {
        console.warn("松手模式超时 60 秒，自动取消");
        setIsSubmitting(true);
        try {
          await invoke("cancel_locked_recording");
          setIsSubmitting(false);
        } catch (e) {
          console.error("取消锁定录音失败:", e);
          setIsSubmitting(false);
        }
      }, 60000);
      return () => clearTimeout(timeout);
    }
  }, [isLocked, isSubmitting]);

  const handleFinish = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    // 300ms debounce: prevent rapid duplicate clicks
    if (submitDebounceRef.current) clearTimeout(submitDebounceRef.current);
    submitDebounceRef.current = setTimeout(() => {
      submitDebounceRef.current = null;
    }, 300);
    try {
      await invoke("finish_locked_recording");
      setIsLocked(false);
      setIsSubmitting(false);
    } catch (e) {
      console.error("完成录音失败:", e);
      setIsSubmitting(false);
    }
  };

  const handleCancel = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    // 300ms debounce: prevent rapid duplicate clicks
    if (submitDebounceRef.current) clearTimeout(submitDebounceRef.current);
    submitDebounceRef.current = setTimeout(() => {
      submitDebounceRef.current = null;
    }, 300);
    try {
      await invoke("cancel_locked_recording");
      setIsSubmitting(false);
    } catch (e) {
      console.error("取消录音失败:", e);
      setIsSubmitting(false);
    }
  };

  // Cleanup debounce timer on unmount
  useEffect(() => {
    return () => {
      if (submitDebounceRef.current) {
        clearTimeout(submitDebounceRef.current);
      }
    };
  }, []);

  return (
    <div
      className={`overlay-root ${theme === "dark" ? "theme-dark" : "theme-light"}`}
      style={status === "transcribing" ? { pointerEvents: 'none' } : undefined}
    >
      {status === "results" ? (
        <div className={`overlay-pill overlay-pill-results`}>
          {/* 渐进式结果面板：使用与 ResultList 完全相同的样式 */}
          {presetResults.length > 0 ? (
            <PresetProgressList
              items={presetResults}
              selectedIndex={selectedIndex}
              onSelect={setSelectedIndex}
              onConfirm={handleSelectPresetResult}
              disabled={isSubmitting}
            />
          ) : (
            <ResultList
              items={resultItems}
              selectedIndex={selectedIndex}
              onSelect={setSelectedIndex}
              onConfirm={confirmResult}
            />
          )}
        </div>
      ) : (
        <div className={`overlay-pill ${isLocked ? 'overlay-pill-locked' : ''}`}
             style={enableLiveTranscript && status === "recording" ? { paddingTop: '8px', paddingBottom: '8px', height: 'auto', minHeight: '48px' } : {}}>
          {status === "recording" ? (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '4px', width: '100%' }}>
              {isLocked ? (
                <LockedControls
                  onFinish={handleFinish}
                  onCancel={handleCancel}
                  level={audioLevel}
                  time={animationTime}
                  disabled={isSubmitting}
                />
              ) : (
                <WaveformBars level={audioLevel} time={animationTime} />
              )}
              {enableLiveTranscript && liveTranscript && (
                <div className="live-transcript-text">{liveTranscript}</div>
              )}
            </div>
          ) : (
            <LoadingIndicator />
          )}
        </div>
      )}
    </div>
  );
}