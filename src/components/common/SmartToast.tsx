import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";

export type SmartToastProps = {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  variant?: "success" | "error";
  autoHideMs?: number;
};

export function SmartToast({
  open,
  onClose,
  children,
  variant = "success",
  autoHideMs = 5000,
}: SmartToastProps) {
  const timerRef = useRef<number | null>(null);
  const mouseEnterRef = useRef(false);

  // Clear any running timer
  const clearTimer = () => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  };

  // Start the auto-dismiss timer (only if not hovering)
  const startTimer = () => {
    clearTimer();
    if (!mouseEnterRef.current) {
      timerRef.current = window.setTimeout(() => {
        onClose();
      }, autoHideMs);
    }
  };

  useEffect(() => {
    if (open) {
      startTimer();
    } else {
      clearTimer();
    }
    return clearTimer;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed bottom-6 right-6 z-[200] max-w-md animate-in slide-in-from-right-4 fade-in duration-300">
      <div
        className={[
          "relative rounded-2xl shadow-xl border p-4 pr-10 text-sm font-medium transition-shadow",
          variant === "success"
            ? "bg-emerald-50 border-emerald-200 text-emerald-800"
            : "bg-red-50 border-red-200 text-red-800",
        ].join(" ")}
        onMouseEnter={() => {
          mouseEnterRef.current = true;
        }}
        onMouseLeave={() => {
          mouseEnterRef.current = false;
          startTimer();
        }}
      >
        {children}

        <button
          type="button"
          onClick={onClose}
          className={[
            "absolute top-3 right-3 p-0.5 rounded-full transition-colors",
            variant === "success"
              ? "text-emerald-400 hover:text-emerald-600 hover:bg-emerald-100"
              : "text-red-400 hover:text-red-600 hover:bg-red-100",
          ].join(" ")}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}