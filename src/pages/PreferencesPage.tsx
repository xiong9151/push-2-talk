import { Download, Power, RefreshCw, SlidersHorizontal, VolumeX, GraduationCap, Settings2, HelpCircle, Mic, AlertCircle } from "lucide-react";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppStatus, UpdateStatus, LearningConfig, SharedLlmConfig, AudioDiagnostics } from "../types";
import { Toggle, ThemeSelector, LlmConnectionConfig, Tooltip } from "../components/common";
import { RedDot } from "../components/common/RedDot";
import { SettingsModal } from "../components/modals/SettingsModal";
import { normalizeLearningConfig } from "../constants";

export type PreferencesPageProps = {
  status: AppStatus;

  enableAutostart: boolean;
  onToggleAutostart: () => void;

  enableMuteOtherApps: boolean;
  onSetEnableMuteOtherApps: (next: boolean) => Promise<void>;

  theme: string;
  setTheme: (theme: string) => Promise<void>;

  updateStatus: UpdateStatus;
  updateInfo: { version: string; notes?: string } | null;
  currentVersion: string;
  onCheckUpdate: () => void;
  onDownloadAndInstall: () => void;

  sharedConfig: SharedLlmConfig;
  learningConfig: LearningConfig;
  setLearningConfig: (next: LearningConfig) => void;
  onSetLearningEnabled: (enabled: boolean) => Promise<void>;
  onNavigateToModels?: () => void;
};

export function PreferencesPage({
  status,
  enableAutostart,
  onToggleAutostart,
  enableMuteOtherApps,
  onSetEnableMuteOtherApps,
  theme,
  setTheme,
  updateStatus,
  updateInfo,
  currentVersion,
  onCheckUpdate,
  onDownloadAndInstall,
  sharedConfig,
  learningConfig,
  setLearningConfig,
  onSetLearningEnabled,
  onNavigateToModels,
}: PreferencesPageProps) {
  const canInstallUpdate = updateStatus === "available" || updateStatus === "downloading";

  // 自动学习配置状态
  const learningEnabled = learningConfig.enabled;
  const [learningConfigModalOpen, setLearningConfigModalOpen] = useState(false);

  // 切换自动学习开关
  const handleToggleLearning = async () => {
    const newValue = !learningEnabled;
    const previousLearningConfig = learningConfig;
    const updatedLearningConfig = normalizeLearningConfig({
      ...learningConfig,
      enabled: newValue,
    });
    setLearningConfig(updatedLearningConfig);

    try {
      await onSetLearningEnabled(newValue);
    } catch (error) {
      console.error("保存自动学习配置失败:", error);
      setLearningConfig(previousLearningConfig); // 回滚
    }
  };

  const [audioDebugResult, setAudioDebugResult] = useState<string | null>(null);
  const [audioDebugRunning, setAudioDebugRunning] = useState(false);
  const [audioDebugError, setAudioDebugError] = useState<string | null>(null);

  const handleAudioDebug = async () => {
    setAudioDebugRunning(true);
    setAudioDebugResult(null);
    setAudioDebugError(null);
    try {
      const result = await invoke<AudioDiagnostics>("debug_audio_recording");
      setAudioDebugResult(
        `时长: ${result.duration_secs.toFixed(1)}s\n` +
        `原始 RMS: ${result.raw_rms.toFixed(4)}（说话时应在 0.01~0.3）\n` +
        `原始峰值: ${result.raw_peak.toFixed(4)}\n` +
        `处理后 RMS: ${result.processed_rms.toFixed(4)}\n` +
        `最终增益: ${result.final_gain.toFixed(2)}x\n` +
        `采样率: ${result.device_sample_rate}Hz → ${result.target_sample_rate}Hz\n` +
        `WAV 大小: ${(result.wav_size_bytes / 1024).toFixed(1)}KB\n\n` +
        `📁 录音已保存到桌面\n` +
        `提示：将对录音文件进行检查时，请观察原始 RMS 值。\n` +
        `• 若原始 RMS >= 0.01：麦克风正常，问题在豆包 API\n` +
        `• 若原始 RMS < 0.01：麦克风问题或录音系统故障`
      );
    } catch (err) {
      setAudioDebugError(String(err));
    } finally {
      setAudioDebugRunning(false);
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <SlidersHorizontal size={14} />
          <span>偏好设置</span>
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                enableAutostart
                  ? "bg-[rgba(34,197,94,0.12)] text-green-500"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <Power size={16} />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">开机自启动</div>
              <div className="text-[11px] text-stone-400 font-semibold">系统启动后自动运行</div>
            </div>
          </div>
          <Toggle checked={enableAutostart} onCheckedChange={() => onToggleAutostart()} size="sm" variant="green" />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                enableMuteOtherApps
                  ? "bg-[rgba(217,119,87,0.12)] text-[var(--crail)]"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <VolumeX size={16} />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">录音时静音其他应用</div>
              <div className="text-[11px] text-stone-400 font-semibold">
                {enableMuteOtherApps ? "录音期间自动静音" : "不干预音频"}
              </div>
            </div>
          </div>
          <Toggle
            checked={enableMuteOtherApps}
            onCheckedChange={(next) => {
              void onSetEnableMuteOtherApps(next);
            }}
            disabled={status === "recording" || status === "transcribing"}
            size="sm"
            variant="orange"
          />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                learningEnabled
                  ? "bg-[rgba(120,140,93,0.12)] text-[var(--sage)]"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <GraduationCap size={16} />
            </div>
            <div>
              <div className="flex items-center gap-1.5">
                <div className="text-sm font-bold text-[var(--ink)]">自动词库学习</div>
                <Tooltip content="AI 自动识别语音中的专业术语、人名和地名，学习后会自动添加到个人词库中，提高后续识别准确率。">
                  <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
                </Tooltip>
              </div>
              <div className="text-[11px] text-stone-400 font-semibold">
                {learningEnabled ? "AI 自动识别专业术语" : "手动管理词库"}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {learningEnabled && (
              <button
                onClick={() => setLearningConfigModalOpen(true)}
                className="p-2 rounded-xl text-stone-400 hover:bg-white hover:text-[var(--ink)] hover:shadow-sm border border-transparent hover:border-[var(--stone)] transition-all"
                title="配置自动学习"
              >
                <Settings2 size={18} />
              </button>
            )}
            <div className="h-6 w-px bg-[var(--stone)] mx-1" />
            <Toggle
              checked={learningEnabled}
              onCheckedChange={handleToggleLearning}
              disabled={status === "recording" || status === "transcribing"}
              size="sm"
              variant="green"
            />
          </div>
        </div>

        <SettingsModal
          open={learningConfigModalOpen}
          onDismiss={() => setLearningConfigModalOpen(false)}
          title="自动词库学习配置"
        >
          <div className="space-y-4">
            <div className="p-4 bg-[rgba(120,140,93,0.08)] border border-[rgba(120,140,93,0.15)] rounded-2xl">
              <p className="text-sm text-[var(--ink)] leading-relaxed">
                开启此功能后，AI 将自动分析您的语音输入，识别并提取专业术语、人名和地名，自动添加到您的个人词库中，提高后续识别的准确率。
              </p>
            </div>

            <div className="space-y-2">
              <h4 className="text-xs font-bold text-stone-500 uppercase tracking-widest">LLM 连接配置</h4>
              <LlmConnectionConfig
                sharedConfig={sharedConfig}
                featureName="learning"
                onNavigateToModels={() => {
                  setLearningConfigModalOpen(false);
                  onNavigateToModels?.();
                }}
              />
            </div>
          </div>
        </SettingsModal>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                theme === "light"
                  ? "bg-[rgba(217,119,87,0.12)] text-[var(--crail)]"
                  : "bg-stone-800 text-stone-200",
              ].join(" ")}
            >
              <div className="w-4 h-4 rounded-full border-2 border-current" />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">悬浮窗风格</div>
              <div className="text-[11px] text-stone-400 font-semibold">
                选择录音指示器外观
              </div>
            </div>
          </div>
          <ThemeSelector
            value={theme}
            onChange={(newTheme) => {
              console.log("[PreferencesPage] 切换主题:", newTheme);
              setTheme(newTheme);
            }}
            disabled={status === "recording" || status === "transcribing"}
          />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div>
            <div className="text-sm font-bold text-[var(--ink)]">检查更新</div>
            <div className="text-[11px] text-stone-400 font-semibold">
              {updateStatus === "available" && updateInfo
                ? `发现新版本 v${updateInfo.version}`
                : `当前版本 v${currentVersion}`}
            </div>
          </div>
          <div className="flex items-center gap-2">
            {canInstallUpdate && (
              <button
                onClick={onDownloadAndInstall}
                disabled={updateStatus === "downloading"}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {updateStatus === "downloading" ? <RefreshCw size={14} className="animate-spin" /> : <Download size={14} />}
                更新
              </button>
            )}
            <button
              onClick={onCheckUpdate}
              disabled={updateStatus === "checking" || updateStatus === "downloading"}
              className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
            >
              {updateStatus === "checking" ? <RefreshCw size={14} className="animate-spin" /> : <RefreshCw size={14} />}
              检查
              {updateStatus === "available" && <RedDot size="md" />}
            </button>
          </div>
        </div>

        {/* 🎤 录音诊断 */}
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
            <Mic size={14} />
            <span>录音诊断</span>
          </div>

          <div className="p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm font-bold text-[var(--ink)]">录制测试音频</div>
                <div className="text-[11px] text-stone-400 font-semibold">
                  录制 3 秒音频并保存到桌面，用于诊断"无有效语音"等问题
                </div>
              </div>
              <button
                onClick={handleAudioDebug}
                disabled={audioDebugRunning}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-stone-400 transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {audioDebugRunning ? <RefreshCw size={14} className="animate-spin" /> : <Mic size={14} />}
                {audioDebugRunning ? "录音中..." : "录制 3 秒"}
              </button>
            </div>

            {audioDebugResult && (
              <div className="p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-800 whitespace-pre-line font-mono leading-relaxed">
                {audioDebugResult}
              </div>
            )}

            {audioDebugError && (
              <div className="flex items-center gap-2 p-3 bg-red-50 border border-red-200 rounded-xl text-xs text-red-700">
                <AlertCircle size={14} />
                <span>{audioDebugError}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
