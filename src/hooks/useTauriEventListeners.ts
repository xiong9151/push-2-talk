import type React from "react";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { nanoid } from "nanoid";
import type {
  AppConfig,
  AppStatus,
  AsrConfig,
  AssistantConfig,
  CustomAsrProvider,
  DictionaryEntry,
  DualHotkeyConfig,
  HistoryRecord,
  LearningConfig,
  LlmConfig,
  TranscriptionResult,
  UsageStats,
} from "../types";
import { MAX_HISTORY, DEFAULT_LEARNING_CONFIG, normalizeLearningConfig } from "../constants";
import { saveHistory, loadUsageStats } from "../utils";
import { parseEntry } from "../utils/dictionaryUtils";
import {
  fetchBuiltinDomains,
  normalizeBuiltinDictionaryDomains,
  setBuiltinDomainsSnapshot,
} from "../utils/builtinDictionary";

type UnlistenFn = () => void;

export type UseTauriEventListenersParams = {
  llmConfigRef: React.RefObject<LlmConfig>;
  enablePostProcessRef?: React.RefObject<boolean>;
  enableDictionaryEnhancementRef?: React.RefObject<boolean>;
  setActivePresetNames?: React.Dispatch<React.SetStateAction<string[]>>;

  setStatus: React.Dispatch<React.SetStateAction<AppStatus>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  setTranscript: React.Dispatch<React.SetStateAction<string>>;
  setOriginalTranscript: React.Dispatch<React.SetStateAction<string | null>>;
  setSelectedText: React.Dispatch<React.SetStateAction<string | null>>;
  setCurrentMode: React.Dispatch<React.SetStateAction<string | null>>;
  setAsrTime: React.Dispatch<React.SetStateAction<number | null>>;
  setLlmTime: React.Dispatch<React.SetStateAction<number | null>>;
  setTotalTime: React.Dispatch<React.SetStateAction<number | null>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;

  setApiKey?: React.Dispatch<React.SetStateAction<string>>;
  setFallbackApiKey?: React.Dispatch<React.SetStateAction<string>>;
  setAsrConfig?: React.Dispatch<React.SetStateAction<AsrConfig>>;
  setUseRealtime?: React.Dispatch<React.SetStateAction<boolean>>;
  setEnablePostProcess?: React.Dispatch<React.SetStateAction<boolean>>;
  setEnableDictionaryEnhancement?: React.Dispatch<React.SetStateAction<boolean>>;
  setLlmConfig?: React.Dispatch<React.SetStateAction<LlmConfig>>;
  setAssistantConfig?: React.Dispatch<React.SetStateAction<AssistantConfig>>;
  setLearningConfig?: React.Dispatch<React.SetStateAction<LearningConfig>>;
  setEnableMuteOtherApps?: React.Dispatch<React.SetStateAction<boolean>>;
  setTheme?: React.Dispatch<React.SetStateAction<string>>;
  setCloseAction?: React.Dispatch<React.SetStateAction<"close" | "minimize" | null>>;
  setDictionary?: React.Dispatch<React.SetStateAction<DictionaryEntry[]>>;
  setDualHotkeyConfig?: React.Dispatch<React.SetStateAction<DualHotkeyConfig>>;
  setBuiltinDictionaryDomains?: React.Dispatch<React.SetStateAction<string[]>>;
  setCustomAsrProviders?: React.Dispatch<React.SetStateAction<CustomAsrProvider[]>>;
  onExternalConfigUpdated?: (config: AppConfig) => void;
  onBuiltinDictionaryUpdated?: () => void;

  setHistory: React.Dispatch<React.SetStateAction<HistoryRecord[]>>;
  setUsageStats?: React.Dispatch<React.SetStateAction<UsageStats>>;

  /** 娑﹁壊澶辫触鏃剁殑鍥炶皟锛堢敤浜庢樉绀?Toast 鎻愮ず锛?*/
  onPolishingFailed?: (errorMessage: string) => void;
};

export function useTauriEventListeners({
  llmConfigRef,
  enablePostProcessRef,
  enableDictionaryEnhancementRef,
  setActivePresetNames,
  setStatus,
  setError,
  setTranscript,
  setOriginalTranscript,
  setSelectedText,
  setCurrentMode,
  setAsrTime,
  setLlmTime,
  setTotalTime,
  setShowCloseDialog,
  setApiKey,
  setFallbackApiKey,
  setAsrConfig,
  setUseRealtime,
  setEnablePostProcess,
  setEnableDictionaryEnhancement,
  setLlmConfig,
  setAssistantConfig,
  setLearningConfig,
  setEnableMuteOtherApps,
  setTheme,
  setCloseAction,
  setDictionary,
  setDualHotkeyConfig,
  setBuiltinDictionaryDomains,
  setCustomAsrProviders,
  onExternalConfigUpdated,
  onBuiltinDictionaryUpdated,
  setHistory,
  setUsageStats,
  onPolishingFailed,
}: UseTauriEventListenersParams) {
  useEffect(() => {
    let unlistenFns: UnlistenFn[] = [];
    let cancelled = false;

    const addHistoryRecord = (record: HistoryRecord) => {
      setHistory((prev) => {
        const updated = [record, ...prev].slice(0, MAX_HISTORY);
        saveHistory(updated);
        return updated;
      });
    };

    // 浠庡悗绔噸鏂板姞杞界粺璁℃暟鎹紙鍚庣宸茶嚜鍔ㄦ洿鏂帮級
    const reloadUsageStats = async () => {
      if (!setUsageStats) return;
      try {
        const stats = await loadUsageStats();
        setUsageStats(stats);
      } catch (error) {
        console.error('閲嶆柊鍔犺浇缁熻鏁版嵁澶辫触:', error);
      }
    };

    const setup = async () => {
      // 杈呭姪鍑芥暟锛氭敞鍐岀洃鍚櫒骞舵鏌ュ彇娑堢姸鎬侊紝瑙ｅ喅 StrictMode 绔炴€佹潯浠?
      const registerListener = async <T>(
        event: string,
        handler: (payload: T) => void | Promise<void>,
      ): Promise<boolean> => {
        const unlisten = await listen<T>(event, (e) => {
          void handler(e.payload as T);
        });
        if (cancelled) {
          unlisten();
          return false;
        }
        unlistenFns.push(unlisten);
        return true;
      };

      try {
        if (!(await registerListener("recording_started", () => {
          setStatus("recording");
          setError(null);
        }))) return;

        if (!(await registerListener("recording_stopped", () => {
          setStatus("transcribing");
        }))) return;

        if (!(await registerListener("transcribing", () => {
          setStatus("transcribing");
        }))) return;

        if (!(await registerListener<string>("post_processing", (mode) => {
          if (mode === "polishing") {
            setStatus("polishing");
          } else if (mode === "assistant") {
            setStatus("assistant_processing");
          }
        }))) return;

        if (!(await registerListener<TranscriptionResult>("transcription_complete", (result) => {
          setTranscript(result.text);
          // 鍙鏈?original_text 灏辨樉绀哄弻鏍忥紙鍘熷杞啓 + 娑﹁壊缁撴灉锛?
          setOriginalTranscript(result.original_text || null);
          setSelectedText(result.selected_text || null);
          setCurrentMode(result.mode || null);
          setAsrTime(result.asr_time_ms);
          setLlmTime(result.llm_time_ms);
          setTotalTime(result.total_time_ms);
          setStatus("running");

          // 鍚庣宸茶嚜鍔ㄦ洿鏂扮粺璁℃暟鎹紝鍓嶇鍙渶閲嶆柊鍔犺浇
          reloadUsageStats();

          const llmConfig = llmConfigRef.current;
          const mode = (result.mode as "normal" | "assistant") || null;
          const enablePostProcess = enablePostProcessRef?.current ?? false;
          const enableDictionaryEnhancement = enableDictionaryEnhancementRef?.current ?? false;

          // presetNames 閫昏緫锛?
          // 1. 濡傛灉娌℃湁 original_text锛堟湭鍚敤娑﹁壊锛夛紝涓嶆樉绀轰换浣曟鼎鑹叉爣绛?
          // 2. 濡傛灉鏄?assistant 妯″紡锛屼笉鏄剧ず娑﹁壊鏍囩
          // 3. 濡傛灉寮€鍚簡娑﹁壊锛屾樉绀烘墍鏈夊嬀閫夌殑棰勮鍚嶇О
          // 4. 濡傛灉寮€鍚簡璇嶅簱澧炲己锛屾樉绀?璇嶅簱澧炲己"
          // 5. 鍏朵粬鎯呭喌锛堜粎 TNL 澶勭悊锛夛紝鏄剧ず"鏂囨湰瑙勮寖鍖?
          const hasPolishing = !!result.original_text;
          let presetNames: string[] = [];
          if (hasPolishing && mode !== "assistant") {
            // 只要产生了润色结果，直接显示所有勾选的预设名称
            // 不依赖 enablePostProcess ref（可能因事件时序尚未更新）
            const selectedPresets = llmConfig?.presets.filter(
                (p) => p.selected_for_display ?? true
              ) ?? [];
            if (selectedPresets.length > 0) {
              presetNames = selectedPresets.map((p) => p.name);
            } else if (enableDictionaryEnhancement) {
              presetNames = ["词库增强"];
            } else {
              presetNames = ["文本规范化"];
            }
          }

          setActivePresetNames?.(presetNames);

          addHistoryRecord({
            id: nanoid(8),
            timestamp: Date.now(),
            originalText: result.original_text || result.text,
            // 鍙鏈?original_text 灏辫缃?polishedText
            polishedText: hasPolishing ? result.text : null,
            selectedText: result.selected_text || null,
            presetName: presetNames.join(", ") || null,
            mode,
            asrTimeMs: result.asr_time_ms,
            llmTimeMs: result.llm_time_ms,
            totalTimeMs: result.total_time_ms,
            success: true,
            errorMessage: null,
            tnlDiagnostics: result.tnl_diagnostics,
          });
        }))) return;

        if (!(await registerListener<string>("error", (errMsg) => {
          // 防御性清洗：移除可能泄露的 API key 模式
          const sanitized = errMsg
            .replace(/\b(sk-[a-zA-Z0-9_-]{8,})\b/g, '***')
            .replace(/\b([Bb]earer\s+[a-zA-Z0-9._-]{8,})\b/g, 'Bearer ***')
            .replace(/\b([a-zA-Z0-9+/]{40,})\b/g, '***');
          setError(sanitized);
          setStatus("running");
          setTranscript("");
          setOriginalTranscript(null);
          setSelectedText(null);
          setCurrentMode(null);
          setAsrTime(null);
          setLlmTime(null);
          setTotalTime(null);

          // 娉ㄦ剰锛氬悗绔湪閿欒鎯呭喌涓嬩笉浼氭洿鏂扮粺璁℃暟鎹紙鍙粺璁℃垚鍔熺殑褰曢煶锛?
          // 杩欓噷閲嶆柊鍔犺浇鏄负浜嗕繚鎸乁I鐘舵€佸悓姝ワ紝浣嗘暟鎹笉浼氬彉鍖?
          reloadUsageStats();

          addHistoryRecord({
            id: nanoid(8),
            timestamp: Date.now(),
            originalText: "",
            polishedText: null,
            presetName: null,
            mode: null,
            asrTimeMs: 0,
            llmTimeMs: null,
            totalTimeMs: 0,
            success: false,
            errorMessage: sanitized,
          });
        }))) return;

        if (!(await registerListener("transcription_cancelled", () => {
          setStatus("running");
          setError(null);
          setTranscript("");
          setOriginalTranscript(null);
          setSelectedText(null);
          setCurrentMode(null);
          setAsrTime(null);
          setLlmTime(null);
          setTotalTime(null);
        }))) return;

        if (!(await registerListener<AppConfig>("config_updated", (config) => {
          onExternalConfigUpdated?.(config);

          setApiKey?.(config.dashscope_api_key || "");
          setFallbackApiKey?.(config.siliconflow_api_key || "");
          if (config.asr_config) setAsrConfig?.(config.asr_config);
          setUseRealtime?.(config.use_realtime_asr ?? true);
          setEnablePostProcess?.(config.enable_llm_post_process ?? false);
          setEnableDictionaryEnhancement?.(config.enable_dictionary_enhancement ?? true);
          setLlmConfig?.(config.llm_config || llmConfigRef.current);
          if (config.assistant_config) setAssistantConfig?.(config.assistant_config);
          setLearningConfig?.(
            normalizeLearningConfig(config.learning_config || DEFAULT_LEARNING_CONFIG),
          );
          setEnableMuteOtherApps?.(config.enable_mute_other_apps ?? false);
          setTheme?.(config.theme || "light");

          if (config.dual_hotkey_config) {
            setDualHotkeyConfig?.(config.dual_hotkey_config);
          }

          const nextCloseAction = config.close_action === "close" || config.close_action === "minimize"
            ? config.close_action
            : null;
          setCloseAction?.(nextCloseAction);

          if (setDictionary) {
            const configDictionary = Array.isArray(config.dictionary) ? config.dictionary : [];
            const normalizedDictionary = configDictionary
              .filter((entry) => typeof entry === "string" && entry.trim())
              .map((entry) => parseEntry(entry));
            setDictionary(normalizedDictionary);
          }

          if (setBuiltinDictionaryDomains) {
            setBuiltinDictionaryDomains(
              normalizeBuiltinDictionaryDomains(config.builtin_dictionary_domains || [])
            );
          }

          if (setCustomAsrProviders && config.custom_asr_providers) {
            setCustomAsrProviders(config.custom_asr_providers);
          }
        }))) return;

        if (!(await registerListener("builtin_dictionary_updated", async () => {
          try {
            const domains = await fetchBuiltinDomains();
            setBuiltinDomainsSnapshot(domains);
            setBuiltinDictionaryDomains?.((prev) =>
              normalizeBuiltinDictionaryDomains(prev),
            );
            onBuiltinDictionaryUpdated?.();
          } catch (error) {
            console.error("鍒锋柊鍐呯疆璇嶅簱蹇収澶辫触:", error);
          }
        }))) return;

        // 鐩戝惉娑﹁壊澶辫触浜嬩欢锛堣鐢ㄦ埛鐭ラ亾娑﹁壊灏濊瘯杩囦絾澶辫触浜嗭級
        if (!(await registerListener<string>("polishing_failed", (errorMessage) => {
          console.warn("娑﹁壊澶辫触:", errorMessage);
          // 防御性清洗：移除可能泄露的 API key 模式
          const sanitized = errorMessage
            .replace(/\b(sk-[a-zA-Z0-9_-]{8,})\b/g, '***')
            .replace(/\b([Bb]earer\s+[a-zA-Z0-9._-]{8,})\b/g, 'Bearer ***')
            .replace(/\b([a-zA-Z0-9+/]{40,})\b/g, '***');
          onPolishingFailed?.(sanitized);
        }))) return;

        if (!(await registerListener("close_requested", async () => {
          // 鐐瑰嚮鍙夊彿濮嬬粓鏈€灏忓寲鍒扮郴缁熸墭鐩?
          // 濡傞渶褰诲簳閫€鍑猴紝璇峰湪绯荤粺鎵樼洏鍙抽敭鑿滃崟涓€夋嫨"閫€鍑?
          try {
            await invoke("hide_to_tray");
          } catch {
            // 濡傛灉 hide_to_tray 澶辫触锛屽洖閫€鍒板師鏉ョ殑閫昏緫
            try {
              const config = await invoke<AppConfig>("load_config");
              if (config.close_action === "close") {
                await invoke("quit_app");
              } else {
                setShowCloseDialog(true);
              }
            } catch {
              setShowCloseDialog(true);
            }
          }
        }))) return;
      } catch (err) {
        if (!cancelled) {
          console.error("setupEventListeners failed:", err);
        }
      }
    };

    void setup();

    return () => {
      cancelled = true;
      unlistenFns.forEach((fn) => fn());
      unlistenFns = [];
    };
  }, [
    llmConfigRef,
    setAsrTime,
    setActivePresetNames,
    setCurrentMode,
    setError,
    setHistory,
    setLlmTime,
    setOriginalTranscript,
    setApiKey,
    setFallbackApiKey,
    setAsrConfig,
    setUseRealtime,
    setEnablePostProcess,
    setEnableDictionaryEnhancement,
    setLlmConfig,
    setAssistantConfig,
    setLearningConfig,
    setEnableMuteOtherApps,
    setTheme,
    setCloseAction,
    setDictionary,
    setDualHotkeyConfig,
    setBuiltinDictionaryDomains,
    onExternalConfigUpdated,
    onBuiltinDictionaryUpdated,
    setShowCloseDialog,
    setStatus,
    setTotalTime,
    setTranscript,
  ]);
}


