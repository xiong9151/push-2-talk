import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Plus,
  Trash2,
  Play,
  CheckCircle2,
  AlertCircle,
  Pencil,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import type { CustomAsrProvider, CustomAsrAuthType, CustomAsrMode } from "../types";
import { SmartToast } from "../components/common/SmartToast";

type ToastState = {
  open: boolean;
  variant: "success" | "error";
  message: string;
};

function emptyProvider(): CustomAsrProvider {
  return {
    name: "",
    endpoint: "",
    api_key: "",
    auth_type: "api_key",
    auth_header_name: "",
    model_id: "",
    modes: ["http"],
    language: "auto",
    sample_rate: 16000,
    response_format: "json",
    custom_config: "",
    enabled: true,
  };
}

const AUTH_TYPE_OPTIONS: { value: CustomAsrAuthType; label: string }[] = [
  { value: "api_key", label: "API Key 请求头" },
  { value: "bearer", label: "Bearer Token" },
  { value: "custom_header", label: "自定义请求头" },
];

const MODE_OPTIONS: { value: CustomAsrMode; label: string }[] = [
  { value: "http", label: "HTTP" },
  { value: "realtime", label: "实时（WebSocket）" },
];

export type CustomAsrPageProps = {
  providers: CustomAsrProvider[];
  onChange: (providers: CustomAsrProvider[]) => void;
};

export function CustomAsrPage({ providers, onChange }: CustomAsrPageProps) {
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editingProvider, setEditingProvider] = useState<CustomAsrProvider | null>(null);
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [testingIndex, setTestingIndex] = useState<number | null>(null);
  const [toast, setToast] = useState<ToastState>({ open: false, variant: "success", message: "" });
  const [showAddForm, setShowAddForm] = useState(false);
  const [newProviderForm, setNewProviderForm] = useState<CustomAsrProvider>(emptyProvider);

  const showToast = useCallback((variant: "success" | "error", message: string) => {
    setToast({ open: true, variant, message });
  }, []);

  const closeToast = useCallback(() => {
    setToast((prev: ToastState) => ({ ...prev, open: false }));
  }, []);

  // Start editing a provider
  const handleStartEdit = (index: number) => {
    setEditingIndex(index);
    setEditingProvider({ ...providers[index] });
    setExpandedIndex(index);
  };

  // Save edit
  const handleSaveEdit = () => {
    if (editingIndex === null || !editingProvider) return;
    const updated = [...providers];
    updated[editingIndex] = editingProvider;
    onChange(updated);
    setEditingIndex(null);
    setEditingProvider(null);
    showToast("success", "已更新自定义 ASR 提供商");
  };

  // Cancel edit
  const handleCancelEdit = () => {
    setEditingIndex(null);
    setEditingProvider(null);
  };

  // Delete provider
  const handleDelete = (index: number) => {
    const updated = providers.filter((_, i) => i !== index);
    onChange(updated);
    showToast("success", "已删除提供商");
    if (expandedIndex === index) setExpandedIndex(null);
    if (editingIndex === index) {
      setEditingIndex(null);
      setEditingProvider(null);
    }
  };

  // Add new provider
  const handleAdd = () => {
    const newProvider = emptyProvider();
    onChange([...providers, newProvider]);
    setExpandedIndex(providers.length);
    setEditingIndex(providers.length);
    setEditingProvider(newProvider);
    setShowAddForm(false);
  };

  // Test provider
  const handleTest = async (index: number) => {
    const provider = providers[index];
    // Validate required fields
    if (!provider.name.trim()) {
      showToast("error", "请先填写提供商名称");
      return;
    }
    if (!provider.endpoint.trim()) {
      showToast("error", "请先填写服务端点 URL");
      return;
    }
    if (!provider.model_id.trim()) {
      showToast("error", "请先填写模型 ID");
      return;
    }

    setTestingIndex(index);
    try {
      const result = await invoke<string>("test_custom_asr", { config: provider });
      showToast("success", `测试成功！识别结果：${result}`);
    } catch (err) {
      showToast("error", `测试失败：${String(err)}`);
    } finally {
      setTestingIndex(null);
    }
  };

  // Render mode checkboxes
  const ModeSelector = ({
    value: modeValue,
    onChange: onModeChange,
    disabled,
  }: {
    value: CustomAsrMode[];
    onChange: (v: CustomAsrMode[]) => void;
    disabled?: boolean;
  }) => (
    <div className="flex gap-4">
      {MODE_OPTIONS.map((opt) => {
        const checked = modeValue.includes(opt.value);
        return (
          <label
            key={opt.value}
            className="flex items-center gap-2 text-xs font-medium text-stone-600 cursor-pointer"
          >
            <input
              type="checkbox"
              checked={checked}
              disabled={disabled}
              onChange={() => {
                if (checked) {
                  onModeChange(modeValue.filter((m) => m !== opt.value));
                } else {
                  onModeChange([...modeValue, opt.value]);
                }
              }}
              className="rounded border-stone-300 text-[var(--steel)] focus:ring-[var(--steel)]"
            />
            {opt.label}
          </label>
        );
      })}
    </div>
  );

  // Render a form for a provider (used both for add and edit)
  const ProviderForm = ({
    provider,
    onChange: onFormChange,
    disabled,
  }: {
    provider: CustomAsrProvider;
    onChange: (p: CustomAsrProvider) => void;
    disabled?: boolean;
  }) => (
    <div className="space-y-3">
      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">显示名称</label>
        <input
          type="text"
          value={provider.name}
          disabled={disabled}
          onChange={(e) => onFormChange({ ...provider, name: e.target.value })}
          placeholder="例如：我的 ASR"
          className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
        />
      </div>

      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">服务端点 URL</label>
        <input
          type="text"
          value={provider.endpoint}
          disabled={disabled}
          onChange={(e) => onFormChange({ ...provider, endpoint: e.target.value })}
          placeholder="https://api.example.com/v1/audio/transcriptions"
          className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
        />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">认证方式</label>
          <select
            value={provider.auth_type}
            disabled={disabled}
            onChange={(e) =>
              onFormChange({ ...provider, auth_type: e.target.value as CustomAsrAuthType })
            }
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          >
            {AUTH_TYPE_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">API Key</label>
          <input
            type="password"
            value={provider.api_key}
            disabled={disabled}
            onChange={(e) => onFormChange({ ...provider, api_key: e.target.value })}
            placeholder="sk-..."
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          />
        </div>
      </div>

      {provider.auth_type === "custom_header" && (
        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">自定义认证头名称</label>
          <input
            type="text"
            value={provider.auth_header_name}
            disabled={disabled}
            onChange={(e) => onFormChange({ ...provider, auth_header_name: e.target.value })}
            placeholder="X-API-Key"
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          />
        </div>
      )}

      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">模型 ID</label>
        <input
          type="text"
          value={provider.model_id}
          disabled={disabled}
          onChange={(e) => onFormChange({ ...provider, model_id: e.target.value })}
          placeholder="例如：whisper-1"
          className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
        />
      </div>

      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">支持的传输模式</label>
        <ModeSelector
          value={provider.modes}
          onChange={(modes) => onFormChange({ ...provider, modes })}
          disabled={disabled}
        />
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">语言</label>
          <input
            type="text"
            value={provider.language}
            disabled={disabled}
            onChange={(e) => onFormChange({ ...provider, language: e.target.value })}
            placeholder="auto"
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          />
        </div>

        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">采样率</label>
          <input
            type="number"
            value={provider.sample_rate}
            disabled={disabled}
            onChange={(e) =>
              onFormChange({ ...provider, sample_rate: Number(e.target.value) || 16000 })
            }
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          />
        </div>

        <div className="space-y-2">
          <label className="text-xs font-bold text-stone-500">响应格式</label>
          <input
            type="text"
            value={provider.response_format}
            disabled={disabled}
            onChange={(e) => onFormChange({ ...provider, response_format: e.target.value })}
            placeholder="json"
            className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
          />
        </div>
      </div>

      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">
          自定义配置（JSON，可选）
        </label>
        <textarea
          value={provider.custom_config}
          disabled={disabled}
          onChange={(e) => onFormChange({ ...provider, custom_config: e.target.value })}
          placeholder='{"temperature": 0.0, "max_tokens": 256}'
          rows={3}
          className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60 font-mono resize-y"
        />
      </div>

      <div className="flex items-center gap-2">
        <label className="text-xs font-bold text-stone-500">启用</label>
        <input
          type="checkbox"
          checked={provider.enabled}
          disabled={disabled}
          onChange={(e) => onFormChange({ ...provider, enabled: e.target.checked })}
          className="rounded border-stone-300 text-[var(--steel)] focus:ring-[var(--steel)]"
        />
      </div>
    </div>
  );

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
            <span>自定义 ASR 提供商</span>
          </div>
          <button
            onClick={() => setShowAddForm(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[var(--steel)] text-white text-xs font-bold rounded-xl hover:opacity-90 transition-opacity"
          >
            <Plus size={14} />
            添加提供商
          </button>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--steel)]" />
          <span>
            自定义 ASR 提供商支持 OpenAI 兼容格式的语音识别 API。添加后可在 ASR 面板中切换使用。
          </span>
        </div>

        {providers.length === 0 && !showAddForm && (
          <div className="text-center py-8 text-sm text-stone-400">
            暂无自定义 ASR 提供商，点击上方按钮添加。
          </div>
        )}

        {/* Add form (shown before existing list) */}
        {showAddForm && (
          <div className="p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)] space-y-4">
            <h4 className="text-sm font-bold text-stone-700">添加自定义 ASR 提供商</h4>
            <ProviderForm
              provider={newProviderForm}
              onChange={setNewProviderForm}
            />
            <div className="flex gap-2 pt-2">
              <button
                onClick={() => {
                  handleAdd();
                  setNewProviderForm(emptyProvider());
                }}
                className="px-4 py-2 bg-[var(--steel)] text-white text-xs font-bold rounded-xl hover:opacity-90 transition-opacity"
              >
                添加
              </button>
              <button
                onClick={() => setShowAddForm(false)}
                className="px-4 py-2 bg-white border border-[var(--stone)] text-stone-600 text-xs font-bold rounded-xl hover:bg-stone-50 transition-colors"
              >
                取消
              </button>
            </div>
          </div>
        )}

        {/* Provider list */}
        {providers.map((provider, index) => {
          const isExpanded = expandedIndex === index;
          const isEditing = editingIndex === index;
          const isDisabled = testingIndex !== null;

          return (
            <div
              key={index}
              className="bg-[var(--panel)] rounded-2xl border border-[var(--stone)] overflow-hidden"
            >
              {/* Header */}
              <button
                onClick={() => setExpandedIndex(isExpanded ? null : index)}
                className="w-full flex items-center justify-between px-4 py-3 text-sm font-bold text-stone-700 hover:bg-white/40 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <div
                    className={`w-2 h-2 rounded-full ${provider.enabled ? "bg-emerald-400" : "bg-stone-300"}`}
                  />
                  <span>{provider.name || "(未命名)"}</span>
                  {provider.endpoint && (
                    <span className="text-xs font-normal text-stone-400 truncate max-w-[200px]">
                      {provider.endpoint}
                    </span>
                  )}
                </div>
                {isExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              </button>

              {/* Expanded body */}
              {isExpanded && (
                <div className="px-4 pb-4 space-y-4">
                  {isEditing ? (
                    <>
                      <ProviderForm
                        provider={editingProvider!}
                        onChange={setEditingProvider}
                        disabled={isDisabled}
                      />
                      <div className="flex gap-2 pt-2">
                        <button
                          onClick={handleSaveEdit}
                          disabled={isDisabled}
                          className="px-4 py-2 bg-[var(--steel)] text-white text-xs font-bold rounded-xl hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                          保存
                        </button>
                        <button
                          onClick={handleCancelEdit}
                          disabled={isDisabled}
                          className="px-4 py-2 bg-white border border-[var(--stone)] text-stone-600 text-xs font-bold rounded-xl hover:bg-stone-50 transition-colors disabled:opacity-50"
                        >
                          取消
                        </button>
                      </div>
                    </>
                  ) : (
                    <>
                      {/* Read-only summary */}
                      <div className="grid grid-cols-2 gap-2 text-xs text-stone-600">
                        <div>
                          <span className="font-bold text-stone-500">端点：</span>
                          {provider.endpoint || "-"}
                        </div>
                        <div>
                          <span className="font-bold text-stone-500">模型：</span>
                          {provider.model_id || "-"}
                        </div>
                        <div>
                          <span className="font-bold text-stone-500">认证方式：</span>
                          {AUTH_TYPE_OPTIONS.find((o) => o.value === provider.auth_type)?.label || provider.auth_type}
                        </div>
                        <div>
                          <span className="font-bold text-stone-500">语言：</span>
                          {provider.language}
                        </div>
                        <div>
                          <span className="font-bold text-stone-500">模式：</span>
                          {provider.modes.map((m) => MODE_OPTIONS.find((o) => o.value === m)?.label || m).join(", ")}
                        </div>
                        <div>
                          <span className="font-bold text-stone-500">采样率：</span>
                          {provider.sample_rate} Hz
                        </div>
                      </div>

                      {/* Actions */}
                      <div className="flex gap-2 pt-2">
                        <button
                          onClick={() => handleStartEdit(index)}
                          disabled={isDisabled}
                          className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-[var(--stone)] text-stone-600 text-xs font-bold rounded-xl hover:bg-stone-50 transition-colors disabled:opacity-50"
                        >
                          <Pencil size={12} />
                          编辑
                        </button>
                        <button
                          onClick={() => void handleTest(index)}
                          disabled={isDisabled || !provider.endpoint || !provider.model_id}
                          className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-[var(--stone)] text-stone-600 text-xs font-bold rounded-xl hover:bg-stone-50 transition-colors disabled:opacity-50"
                        >
                          {testingIndex === index ? (
                            <span className="w-3 h-3 border-2 border-stone-400 border-t-transparent rounded-full animate-spin" />
                          ) : (
                            <Play size={12} />
                          )}
                          测试
                        </button>
                        <button
                          onClick={() => handleDelete(index)}
                          disabled={isDisabled}
                          className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-red-200 text-red-500 text-xs font-bold rounded-xl hover:bg-red-50 transition-colors disabled:opacity-50 ml-auto"
                        >
                          <Trash2 size={12} />
                          删除
                        </button>
                      </div>
                    </>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      <SmartToast
        open={toast.open}
        onClose={closeToast}
        variant={toast.variant}
        autoHideMs={5000}
      >
        <div className="flex items-start gap-2">
          {toast.variant === "success" ? (
            <CheckCircle2 size={18} className="flex-shrink-0 mt-0.5 text-emerald-500" />
          ) : (
            <AlertCircle size={18} className="flex-shrink-0 mt-0.5 text-red-500" />
          )}
          <div className="whitespace-pre-wrap break-words">{toast.message}</div>
        </div>
      </SmartToast>
    </div>
  );
}