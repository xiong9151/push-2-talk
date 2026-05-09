import assert from "node:assert/strict";
import test from "node:test";
import {
  buildModelSelectOptions,
  clearPresetOverridesForProvider,
  computeOverrideCommit,
  deriveInvalidPresetIds,
  resolvePresetEffectiveModel,
} from "../src/utils/presetOverride";
import type { LlmConfig, LlmPreset, LlmProvider } from "../src/types";

const provider = (id: string, default_model = `${id}-model`, api_key = `${id}-key`): LlmProvider => ({
  id,
  name: `Provider ${id}`,
  endpoint: `https://${id}.example.com/v1/chat/completions`,
  api_key,
  default_model,
});

const preset = (
  id: string,
  override?: Partial<Pick<LlmPreset, "provider_id" | "model">>,
): LlmPreset => ({
  id,
  name: `Preset ${id}`,
  system_prompt: "",
  provider_id: override?.provider_id,
  model: override?.model,
});

const buildConfig = (
  presets: LlmPreset[],
  providers: LlmProvider[],
  extras: Partial<LlmConfig["shared"]> = {},
): LlmConfig => ({
  shared: {
    providers,
    default_provider_id: providers[0]?.id ?? "",
    polishing_provider_id: undefined,
    assistant_provider_id: undefined,
    learning_provider_id: undefined,
    polishing_model: undefined,
    assistant_model: undefined,
    learning_model: undefined,
    ...extras,
  } as LlmConfig["shared"],
  feature_override: { use_shared: true } as LlmConfig["feature_override"],
  presets,
  active_preset_id: presets[0]?.id ?? "",
});

// ============================================================================
// T13 (kept): model input trim trailing whitespace; empty = inherit/default
// ============================================================================

test("T13 model_input_trim_empty_to_undefined", () => {
  // Trim trailing whitespace
  const trimmed = computeOverrideCommit("p1", "gpt-4o   ");
  assert.equal(trimmed.providerId, "p1");
  assert.equal(trimmed.model, "gpt-4o");

  // Empty after trim → undefined (use provider default)
  const empty = computeOverrideCommit("p1", "   ");
  assert.equal(empty.providerId, "p1");
  assert.equal(empty.model, undefined);

  // Pure empty input → undefined
  const empty2 = computeOverrideCommit("p1", "");
  assert.equal(empty2.providerId, "p1");
  assert.equal(empty2.model, undefined);

  // Normal value preserved
  const normal = computeOverrideCommit("p1", "claude-opus-4-7");
  assert.equal(normal.providerId, "p1");
  assert.equal(normal.model, "claude-opus-4-7");
});

// ============================================================================
// State ④ invariant: provider_id=null forces model=undefined
// (Even if model input had a value, selecting "跟随默认" wipes it)
// ============================================================================

test("invariant: provider_id null forces model undefined (state 4 banned)", () => {
  const result = computeOverrideCommit(null, "leftover-model-text");
  assert.equal(result.providerId, undefined);
  assert.equal(result.model, undefined);
});

// ============================================================================
// T14: derived invalidation set includes dangling provider_id
// ============================================================================

test("T14 derived_invalidation_set_includes_dangling_provider_id", () => {
  const providers = [provider("prov-a"), provider("prov-b")];
  const presets = [
    preset("p1"),
    preset("p2", { provider_id: "prov-a" }),
    preset("p3", { provider_id: "prov-deleted" }),
    preset("p4", { provider_id: "prov-b", model: "custom" }),
    preset("p5", { provider_id: "another-ghost" }),
  ];

  const invalid = deriveInvalidPresetIds(presets, providers);

  assert.equal(invalid.size, 2);
  assert.ok(invalid.has("p3"));
  assert.ok(invalid.has("p5"));
  assert.ok(!invalid.has("p1"));
  assert.ok(!invalid.has("p2"));
  assert.ok(!invalid.has("p4"));
});

// ============================================================================
// T16: delete provider clears referencing preset overrides (Plan A core)
// ============================================================================

test("T16 delete_provider_clears_referencing_preset_overrides", () => {
  const presets: LlmPreset[] = [
    preset("p1"),
    preset("p2", { provider_id: "doomed", model: "custom-model" }),
    preset("p3", { provider_id: "other" }),
    preset("p4", { provider_id: "doomed" }),
  ];

  const result = clearPresetOverridesForProvider(presets, "doomed");

  assert.equal(result[0].provider_id, undefined);
  assert.equal(result[1].provider_id, undefined);
  assert.equal(result[1].model, undefined, "model 必须一并清空");
  assert.equal(result[2].provider_id, "other", "无关 preset 不动");
  assert.equal(result[3].provider_id, undefined);

  // Immutable contract
  assert.equal(presets[1].provider_id, "doomed", "原数组不应被修改");
});

test("T16b delete_unrelated_provider_id_is_noop", () => {
  const presets: LlmPreset[] = [preset("p1"), preset("p2", { provider_id: "prov-x" })];
  const result = clearPresetOverridesForProvider(presets, "never-existed");
  assert.equal(result[1].provider_id, "prov-x");
});

// ============================================================================
// resolvePresetEffectiveModel — three resolution states
// ============================================================================

test("resolvePresetEffectiveModel — inherit when preset has no override", () => {
  const config = buildConfig([preset("p1")], [provider("prov-a", "model-a")]);
  const eff = resolvePresetEffectiveModel(config.presets[0], config);
  assert.equal(eff.kind, "inherit");
  if (eff.kind === "inherit") {
    assert.equal(eff.model, "model-a");
    assert.equal(eff.providerName, "Provider prov-a");
  }
});

test("resolvePresetEffectiveModel — direct when preset.provider_id valid", () => {
  const config = buildConfig(
    [preset("p1", { provider_id: "prov-b", model: "custom-x" })],
    [provider("prov-a"), provider("prov-b", "default-b")],
  );
  const eff = resolvePresetEffectiveModel(config.presets[0], config);
  assert.equal(eff.kind, "direct");
  if (eff.kind === "direct") {
    assert.equal(eff.providerId, "prov-b");
    assert.equal(eff.model, "custom-x");
  }
});

test("resolvePresetEffectiveModel — direct uses provider.default_model when preset.model empty", () => {
  const config = buildConfig(
    [preset("p1", { provider_id: "prov-b" })],
    [provider("prov-a"), provider("prov-b", "default-b")],
  );
  const eff = resolvePresetEffectiveModel(config.presets[0], config);
  assert.equal(eff.kind, "direct");
  if (eff.kind === "direct") {
    assert.equal(eff.model, "default-b");
  }
});

test("resolvePresetEffectiveModel — dangling when provider deleted", () => {
  const config = buildConfig(
    [preset("p1", { provider_id: "ghost" })],
    [provider("prov-a", "fallback-model")],
  );
  const eff = resolvePresetEffectiveModel(config.presets[0], config);
  assert.equal(eff.kind, "dangling");
  if (eff.kind === "dangling") {
    assert.equal(eff.danglingProviderId, "ghost");
    assert.equal(eff.fallbackModel, "fallback-model");
  }
});

test("resolvePresetEffectiveModel — inherit honors shared.polishing_model when set", () => {
  const config = buildConfig(
    [preset("p1")],
    [provider("prov-a", "default-a")],
    { polishing_model: "shared-polishing-override" },
  );
  const eff = resolvePresetEffectiveModel(config.presets[0], config);
  assert.equal(eff.kind, "inherit");
  if (eff.kind === "inherit") {
    assert.equal(eff.model, "shared-polishing-override");
  }
});

// ============================================================================
// buildModelSelectOptions — dropdown content for inline model select
// ============================================================================

test("buildModelSelectOptions — inherit option always first with effective model in label", () => {
  const config = buildConfig(
    [preset("p1")],
    [provider("prov-a", "model-a"), provider("prov-b", "model-b")],
  );
  const opts = buildModelSelectOptions(config.presets[0], config.shared.providers, config);

  assert.equal(opts[0].value, "");
  assert.match(opts[0].label, /跟随润色默认/);
  assert.match(opts[0].label, /Provider prov-a.*model-a/);
  assert.equal(opts[1].value, "prov-a");
  assert.equal(opts[2].value, "prov-b");
  assert.equal(opts.length, 3);
});

test("buildModelSelectOptions — providers with empty api_key get suffix", () => {
  const config = buildConfig(
    [preset("p1")],
    [provider("prov-a", "model-a", ""), provider("prov-b")],
  );
  const opts = buildModelSelectOptions(config.presets[0], config.shared.providers, config);

  const provA = opts.find((o) => o.value === "prov-a");
  assert.ok(provA);
  assert.match(provA!.label, /缺 API Key/);

  const provB = opts.find((o) => o.value === "prov-b");
  assert.ok(provB);
  assert.doesNotMatch(provB!.label, /缺 API Key/);
});

test("buildModelSelectOptions — dangling provider_id appears as disabled option", () => {
  const config = buildConfig(
    [preset("p1", { provider_id: "ghost-prov" })],
    [provider("prov-a")],
  );
  const opts = buildModelSelectOptions(config.presets[0], config.shared.providers, config);

  // Last option should be the dangling reference
  const dangling = opts.find((o) => o.value === "ghost-prov");
  assert.ok(dangling, "dangling provider must appear in options");
  assert.equal(dangling!.disabled, true);
  assert.match(dangling!.label, /已删除/);
});

test("buildModelSelectOptions — no dangling entry when provider_id is valid", () => {
  const config = buildConfig(
    [preset("p1", { provider_id: "prov-a" })],
    [provider("prov-a")],
  );
  const opts = buildModelSelectOptions(config.presets[0], config.shared.providers, config);

  // Only inherit + prov-a, no extra dangling
  assert.equal(opts.length, 2);
  assert.ok(!opts.some((o) => o.disabled));
});
