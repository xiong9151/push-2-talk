import type { Dispatch, SetStateAction } from "react";
import { AlertCircle, Sparkles } from "lucide-react";
import type { AsrConfig, AsrProvider, CustomAsrProvider } from "../types";
import { ASR_PROVIDERS } from "../constants";
import { ApiKeyInput, Toggle, ConfigSelect } from "../components/common";
import { useConfigSave } from "../contexts/ConfigSaveContext";

export type AsrPageProps = {
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;

  isRunning: boolean;

  customAsrProviders?: CustomAsrProvider[];
};

export function AsrPage({
  asrConfig,
  setAsrConfig,
  showApiKey,
  setShowApiKey,
  isRunning,
  customAsrProviders = [],
}: AsrPageProps) {
  const { saveImmediately, isExternalSyncing } = useConfigSave();
  // 只在外部配置同步时传入状态，用户本地操作让各组件自行管理 internalStatus
  const externalOnlySyncStatus = isExternalSyncing
    ? ("syncing" as const)
    : undefined;

  const enabledCustomProviders = customAsrProviders.filter((p) => p.enabled);
  const hasCustomProviders = enabledCustomProviders.length > 0;

  // 主模型选项
  const activeProviderOptions: { value: string; label: string }[] = [
    { value: "qwen", label: ASR_PROVIDERS.qwen.name },
    { value: "doubao", label: ASR_PROVIDERS.doubao.name },
    { value: "doubao_ime", label: ASR_PROVIDERS.doubao_ime.name },
  ];
  if (hasCustomProviders) {
    activeProviderOptions.push({ value: "custom", label: "自定义 ASR" });
  }

  // 备用模型选项
  const fallbackProviderOptions: { value: string; label: string }[] = [
    { value: "siliconflow", label: ASR_PROVIDERS.siliconflow.name },
  ];
  if (hasCustomProviders) {
    enabledCustomProviders.forEach((p) => {
      fallbackProviderOptions.push({ value: `custom:${p.name}`, label: `${p.name} (自定义)` });
    });
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <span>识别引擎</span>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--steel)]" />
          <span>ASR 用于语音转文字：千问 / 豆包 + 硅基备用。</span>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">主模型</h4>
          <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
            <div className="space-y-2">
              <label className="text-xs font-bold text-stone-500">服务商</label>
              <ConfigSelect
                value={asrConfig.selection.active_provider}
                onChange={(newProvider) => {
                  setAsrConfig((prev) => ({
                    ...prev,
                    selection: { ...prev.selection, active_provider: newProvider as AsrProvider },
                  }));
                }}
                onCommit={async (newProvider) => {
                  await saveImmediately({
                    asrConfig: {
                      ...asrConfig,
                      selection: { ...asrConfig.selection, active_provider: newProvider as AsrProvider },
                    },
                  });
                }}
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={activeProviderOptions}
              />
            </div>

            {asrConfig.selection.active_provider === "qwen" && (
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">API Key</label>
                <ApiKeyInput
                  value={asrConfig.credentials.qwen_api_key}
                  onChange={(value) => {
                    setAsrConfig((prev) => ({
                      ...prev,
                      credentials: { ...prev.credentials, qwen_api_key: value },
                    }));
                  }}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                  placeholder="sk-..."
                />
              </div>
            )}

            {asrConfig.selection.active_provider === "doubao" && (
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">APP ID</label>
                  <input
                    type="text"
                    value={asrConfig.credentials.doubao_app_id}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({
                        ...prev,
                        credentials: { ...prev.credentials, doubao_app_id: value },
                      }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">Access Token</label>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={asrConfig.credentials.doubao_access_token}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({
                        ...prev,
                        credentials: { ...prev.credentials, doubao_access_token: value },
                      }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
              </div>
            )}

            {asrConfig.selection.active_provider === "custom" && (
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">自定义提供商</label>
                <select
                  value={asrConfig.selection.active_custom_asr_name || ""}
                  disabled={isRunning}
                  onChange={(e) => {
                    const name = e.target.value;
                    setAsrConfig((prev) => ({
                      ...prev,
                      selection: { ...prev.selection, active_custom_asr_name: name },
                    }));
                  }}
                  className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                >
                  <option value="">-- 请选择 --</option>
                  {enabledCustomProviders.map((p) => (
                    <option key={p.name} value={p.name}>{p.name}</option>
                  ))}
                </select>
              </div>
            )}

            {asrConfig.selection.active_provider === "doubao_ime" && (
              <div className="flex items-center gap-2 p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-700">
                <Sparkles size={14} className="flex-shrink-0" />
                <span>无需配置，首次使用时自动注册设备凭据。</span>
              </div>
            )}

            <div className="text-xs text-stone-400 font-semibold">
              模型：{ASR_PROVIDERS[asrConfig.selection.active_provider].model}
            </div>

            <div className="space-y-2">
              <label className="text-xs font-bold text-stone-500">识别语言</label>
              <ConfigSelect
                value={asrConfig.language_mode}
                onChange={(mode) => {
                  setAsrConfig((prev) => ({
                    ...prev,
                    language_mode: mode,
                  }));
                }}
                onCommit={async (mode) => {
                  await saveImmediately({
                    asrConfig: {
                      ...asrConfig,
                      language_mode: mode,
                    },
                  });
                }}
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={[
                  { value: "auto", label: "自动识别（推荐）" },
                  { value: "zh", label: "中文优先" },
                ]}
              />
            </div>
          </div>
        </div>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-bold text-stone-700">备用模型</h4>
            <Toggle
              checked={asrConfig.selection.enable_fallback}
              onCheckedChange={(next) =>
                setAsrConfig((prev) => ({
                  ...prev,
                  selection: {
                    ...prev.selection,
                    enable_fallback: next,
                    fallback_provider: next ? "siliconflow" : null,
                  },
                }))
              }
              disabled={isRunning || asrConfig.selection.active_provider === 'doubao_ime'}
              size="xs"
              variant="orange"
            />
          </div>

          {asrConfig.selection.active_provider === 'doubao_ime' && (
            <div className="flex items-center gap-2 p-3 bg-stone-50 border border-stone-200 rounded-xl text-xs text-stone-500">
              <span>豆包输入法模式暂不支持备用模型配置</span>
            </div>
          )}

          {asrConfig.selection.enable_fallback && asrConfig.selection.active_provider !== 'doubao_ime' && (
            <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">服务商</label>
                <select
                  value={
                    asrConfig.selection.fallback_provider === "custom"
                      ? `custom:${asrConfig.selection.fallback_custom_asr_name || ""}`
                      : asrConfig.selection.fallback_provider || "siliconflow"
                  }
                  disabled={isRunning}
                  onChange={(e) => {
                    const val = e.target.value;
                    if (val.startsWith("custom:")) {
                      const name = val.slice(7);
                      setAsrConfig((prev) => ({
                        ...prev,
                        selection: {
                          ...prev.selection,
                          fallback_provider: "custom" as AsrProvider,
                          fallback_custom_asr_name: name,
                        },
                      }));
                    } else {
                      setAsrConfig((prev) => ({
                        ...prev,
                        selection: {
                          ...prev.selection,
                          fallback_provider: val as AsrProvider,
                          fallback_custom_asr_name: "",
                        },
                      }));
                    }
                  }}
                  className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                >
                  {fallbackProviderOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>{opt.label}</option>
                  ))}
                </select>
              </div>
              <div className="text-xs text-stone-400 font-semibold">
                模型：{asrConfig.selection.fallback_provider === "custom"
                  ? asrConfig.selection.fallback_custom_asr_name || "未选择"
                  : ASR_PROVIDERS[asrConfig.selection.fallback_provider || "siliconflow"].model
                }
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
