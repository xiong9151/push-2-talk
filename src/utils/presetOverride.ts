import type { LlmConfig, LlmPreset, LlmProvider } from "../types";

/**
 * Effective resolution of a preset's runtime LLM target.
 * Mirrors the backend `LlmConfig::resolve_polishing` decision tree.
 *
 * - `inherit`: preset has no provider_id → uses polishing default
 * - `direct`: preset.provider_id is valid → uses preset.model || provider.default_model
 * - `dangling`: preset.provider_id points to a deleted provider → falls back to default
 */
export type PresetEffectiveModel =
  | { kind: "inherit"; providerId?: string; providerName: string; model: string }
  | { kind: "direct"; providerId: string; providerName: string; model: string }
  | { kind: "dangling"; danglingProviderId: string; fallbackModel: string };

/**
 * Resolve which provider+model a preset actually uses at runtime.
 * Used by the inline model select to display the "跟随润色默认（GLM-4...）" label
 * and to flag dangling references.
 */
export function resolvePresetEffectiveModel(
  preset: LlmPreset,
  llmConfig: LlmConfig,
): PresetEffectiveModel {
  const providers = llmConfig.shared.providers;
  const polishingDefaultProviderId =
    llmConfig.shared.polishing_provider_id || llmConfig.shared.default_provider_id;
  const defaultProvider =
    providers.find((p) => p.id === polishingDefaultProviderId) || providers[0];
  const defaultModel =
    llmConfig.shared.polishing_model ||
    defaultProvider?.default_model ||
    "(未配置)";

  if (preset.provider_id) {
    const targetProvider = providers.find((p) => p.id === preset.provider_id);
    if (!targetProvider) {
      return {
        kind: "dangling",
        danglingProviderId: preset.provider_id,
        fallbackModel: defaultModel,
      };
    }
    return {
      kind: "direct",
      providerId: targetProvider.id,
      providerName: targetProvider.name,
      model: preset.model || targetProvider.default_model || "(未设置)",
    };
  }

  return {
    kind: "inherit",
    providerId: defaultProvider?.id,
    providerName: defaultProvider?.name ?? "(未配置)",
    model: defaultModel,
  };
}

/**
 * Derive the set of preset IDs whose `provider_id` points to a deleted provider.
 * Boundary defense (manual config edits or migration races).
 */
export function deriveInvalidPresetIds(
  presets: ReadonlyArray<LlmPreset>,
  providers: ReadonlyArray<{ id: string }>,
): Set<string> {
  const providerIds = new Set(providers.map((p) => p.id));
  return new Set(
    presets
      .filter((p) => p.provider_id && !providerIds.has(p.provider_id))
      .map((p) => p.id),
  );
}

/**
 * Compute the values to commit when the user changes a preset's model selection.
 *
 * Invariant guard (state ④ ban): when `selectedProviderId === null`, model is
 * forced to `undefined` regardless of the textbox value.
 *
 * Trim handling: trailing whitespace stripped; empty string maps to `undefined`
 * (use the provider's default_model).
 */
export function computeOverrideCommit(
  selectedProviderId: string | null,
  modelInput: string,
): { providerId: string | undefined; model: string | undefined } {
  if (selectedProviderId === null) {
    return { providerId: undefined, model: undefined };
  }
  const trimmed = modelInput.trimEnd();
  return {
    providerId: selectedProviderId,
    model: trimmed === "" ? undefined : trimmed,
  };
}

/**
 * R5.3 cascade (Plan A): when a provider is deleted, every preset that
 * referenced it must have its `provider_id` and `model` cleared, mirroring
 * the existing shared-layer cleanup (polishing_provider_id etc.).
 *
 * Returns a new array; never mutates the input.
 */
export function clearPresetOverridesForProvider<
  T extends Pick<LlmPreset, "provider_id" | "model">,
>(presets: ReadonlyArray<T>, deletedProviderId: string): T[] {
  return presets.map((p) =>
    p.provider_id === deletedProviderId
      ? { ...p, provider_id: undefined, model: undefined }
      : p,
  );
}

/**
 * Build the option list for the inline model select dropdown.
 * - Always includes "跟随润色默认（<name>）" as first option (value="")
 * - All providers with their names
 * - If preset.provider_id points to a deleted provider, append a disabled
 *   "(已删除) <id>" entry with that id as value (so the select can render
 *   the current dangling state without auto-mutating user data)
 */
export type ModelSelectOption = {
  value: string;
  label: string;
  disabled?: boolean;
};

export function buildModelSelectOptions(
  preset: LlmPreset,
  providers: ReadonlyArray<LlmProvider>,
  llmConfig: LlmConfig,
): ModelSelectOption[] {
  const effective = resolvePresetEffectiveModel(preset, llmConfig);
  const inheritLabel =
    effective.kind === "inherit"
      ? `跟随润色默认（${effective.providerName} · ${effective.model}）`
      : `跟随润色默认（${
          llmConfig.shared.providers.find(
            (p) =>
              p.id ===
              (llmConfig.shared.polishing_provider_id ||
                llmConfig.shared.default_provider_id),
          )?.name ?? "未配置"
        }）`;

  const opts: ModelSelectOption[] = [{ value: "", label: inheritLabel }];

  for (const p of providers) {
    const apiKeyMissing = !p.api_key.trim();
    opts.push({
      value: p.id,
      label: `${p.name}${apiKeyMissing ? "（缺 API Key）" : ""}`,
    });
  }

  // Dangling reference: surface the current state without mutating
  if (
    preset.provider_id &&
    !providers.find((p) => p.id === preset.provider_id)
  ) {
    opts.push({
      value: preset.provider_id,
      label: `(已删除) ${preset.provider_id}`,
      disabled: true,
    });
  }

  return opts;
}
