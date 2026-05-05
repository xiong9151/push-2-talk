import { Sparkles } from "lucide-react";
import type { TnlCandidateDecision, TnlDiagnostics } from "../../types";

const decisionLabels: Record<TnlCandidateDecision, string> = {
  applied_local: "本地应用",
  pending_llm: "待仲裁",
  applied_llm: "模型确认",
  rejected_local: "本地保留",
  rejected_llm: "模型保留",
  skipped_disabled: "增强关闭",
  skipped_no_processor: "未配置模型",
  skipped_timeout: "超时保留",
  skipped_error: "失败保留",
  skipped_limit: "超限保留",
};

const sourceLabels: Record<string, string> = {
  spoken_symbol: "口语符号",
  dictionary_exact: "词库精确",
  dictionary_fuzzy: "词库模糊",
  dictionary_pinyin: "拼音匹配",
  dictionary_phonetic: "音近匹配",
  letter_merge: "字母合并",
};

const correctedDecisions = new Set<TnlCandidateDecision>([
  "applied_local",
  "applied_llm",
]);

export function getTnlCorrectionCount(diagnostics?: TnlDiagnostics): number {
  return diagnostics?.candidates.filter((c) => correctedDecisions.has(c.decision)).length ?? 0;
}

export function TnlDiagnosticsBadge({ diagnostics }: { diagnostics?: TnlDiagnostics }) {
  const count = getTnlCorrectionCount(diagnostics);
  if (count === 0) return null;

  return (
    <span className="text-[10px] bg-[rgba(106,155,204,0.12)] text-[var(--steel)] px-1.5 py-0.5 rounded flex items-center gap-1">
      <Sparkles size={10} />
      热词修正 {count}
    </span>
  );
}

export function TnlDiagnosticsSummary({ diagnostics }: { diagnostics?: TnlDiagnostics }) {
  const visibleCandidates = diagnostics?.candidates.filter((c) => c.decision !== "pending_llm") ?? [];
  if (visibleCandidates.length === 0) return null;

  return (
    <div className="mt-3 rounded-xl border border-[rgba(106,155,204,0.18)] bg-[rgba(106,155,204,0.06)] px-3 py-2.5">
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="text-[11px] font-semibold text-[var(--steel)] flex items-center gap-1.5">
          <Sparkles size={12} />
          热词候选
        </div>
        {diagnostics?.arbitration?.elapsed_ms != null && (
          <span className="text-[10px] text-stone-400">
            仲裁 {diagnostics.arbitration.elapsed_ms}ms
          </span>
        )}
      </div>
      <div className="space-y-1.5">
        {visibleCandidates.slice(0, 4).map((candidate) => (
          <div key={candidate.id} className="flex items-start justify-between gap-2 text-[11px]">
            <div className="min-w-0 flex-1">
              <span className="text-stone-500 break-words">{candidate.original}</span>
              <span className="mx-1 text-stone-300">-&gt;</span>
              <span className="font-semibold text-stone-800 break-words">{candidate.target}</span>
            </div>
            <div className="shrink-0 text-right text-stone-400">
              <div>{decisionLabels[candidate.decision]}</div>
              <div>{sourceLabels[candidate.source] || candidate.source}</div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
