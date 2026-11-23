import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from "lucide-react";
import {
  type PropsWithChildren,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { cn } from "../lib/utils.js";

export type ToastVariant = "info" | "success" | "warning" | "error";

export interface ToastInput {
  message: string;
  title?: string;
  variant?: ToastVariant;
  /** Auto-dismiss delay in milliseconds. Set to 0 to disable auto-dismiss. */
  durationMs?: number;
}

export interface ToastRecord {
  id: string;
  message: string;
  title?: string;
  variant: ToastVariant;
  durationMs: number;
}

export interface ToastContextValue {
  toast: (input: ToastInput) => string;
  dismiss: (id: string) => void;
  dismissAll: () => void;
}

const DEFAULT_DURATION_BY_VARIANT: Record<ToastVariant, number> = {
  info: 4000,
  success: 4000,
  warning: 5500,
  error: 6500,
};

const MAX_VISIBLE_TOASTS = 5;

const ToastContext = createContext<ToastContextValue | null>(null);

const VARIANT_STYLES: Record<
  ToastVariant,
  { container: string; icon: string; bar: string; Icon: typeof Info }
> = {
  info: {
    container: "border-border bg-card text-card-foreground",
    icon: "text-sky-600 dark:text-sky-400",
    bar: "bg-sky-500/70",
    Icon: Info,
  },
  success: {
    container: "border-emerald-500/30 bg-card text-card-foreground",
    icon: "text-emerald-600 dark:text-emerald-400",
    bar: "bg-emerald-500/80",
    Icon: CheckCircle2,
  },
  warning: {
    container: "border-amber-500/40 bg-card text-card-foreground",
    icon: "text-amber-600 dark:text-amber-400",
    bar: "bg-amber-500/80",
    Icon: AlertTriangle,
  },
  error: {
    container: "border-destructive/40 bg-card text-card-foreground",
    icon: "text-destructive",
    bar: "bg-destructive/80",
    Icon: AlertCircle,
  },
};

interface ToastItemProps {
  item: ToastRecord;
  onDismiss: (id: string) => void;
  closeLabel: string;
}

function ToastItem({ item, onDismiss, closeLabel }: ToastItemProps) {
  const [paused, setPaused] = useState(false);
  const remainingRef = useRef(item.durationMs);
  const startedAtRef = useRef<number | null>(null);

  useEffect(() => {
    if (item.durationMs <= 0 || paused) {
      return;
    }

    startedAtRef.current = Date.now();
    const timer = window.setTimeout(() => {
      onDismiss(item.id);
    }, remainingRef.current);

    return () => {
      window.clearTimeout(timer);
      if (startedAtRef.current !== null) {
        const elapsed = Date.now() - startedAtRef.current;
        remainingRef.current = Math.max(0, remainingRef.current - elapsed);
        startedAtRef.current = null;
      }
    };
  }, [item.id, item.durationMs, paused, onDismiss]);

  const style = VARIANT_STYLES[item.variant];
  const Icon = style.Icon;
  const role = item.variant === "error" || item.variant === "warning" ? "alert" : "status";

  return (
    <div
      role={role}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
      className={cn(
        "os-toast-enter pointer-events-auto relative w-full max-w-sm overflow-hidden rounded-lg border shadow-lg",
        style.container,
      )}
    >
      <div className="flex items-start gap-3 px-3.5 py-3">
        <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", style.icon)} aria-hidden="true" />
        <div className="min-w-0 flex-1 text-sm">
          {item.title && <p className="font-semibold leading-tight">{item.title}</p>}
          <p
            className={cn(
              "break-words leading-snug",
              item.title ? "mt-0.5 text-muted-foreground" : "text-foreground",
            )}
          >
            {item.message}
          </p>
        </div>
        <button
          type="button"
          onClick={() => onDismiss(item.id)}
          aria-label={closeLabel}
          className="inline-flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
        >
          <X className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>
      {item.durationMs > 0 && (
        <div className="h-0.5 w-full bg-muted/60">
          <div
            className={cn("os-toast-progress h-full w-full", style.bar)}
            style={{
              animationDuration: `${item.durationMs}ms`,
              animationPlayState: paused ? "paused" : "running",
            }}
          />
        </div>
      )}
    </div>
  );
}

export interface ToastProviderProps extends PropsWithChildren {
  closeLabel?: string;
}

export function ToastProvider({ children, closeLabel = "Close" }: ToastProviderProps) {
  const [toasts, setToasts] = useState<ToastRecord[]>([]);
  const nextIdRef = useRef(0);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const dismissAll = useCallback(() => {
    setToasts([]);
  }, []);

  const toast = useCallback((input: ToastInput) => {
    nextIdRef.current += 1;
    const id = `toast-${nextIdRef.current}`;
    const variant = input.variant ?? "info";
    const durationMs =
      input.durationMs !== undefined ? input.durationMs : DEFAULT_DURATION_BY_VARIANT[variant];

    const record: ToastRecord = {
      id,
      message: input.message,
      title: input.title,
      variant,
      durationMs,
    };

    setToasts((prev) => [...prev.slice(-(MAX_VISIBLE_TOASTS - 1)), record]);
    return id;
  }, []);

  const value = useMemo<ToastContextValue>(
    () => ({ toast, dismiss, dismissAll }),
    [toast, dismiss, dismissAll],
  );

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div
        aria-live="polite"
        aria-atomic="false"
        className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-full max-w-sm flex-col gap-2"
      >
        {toasts.map((item) => (
          <ToastItem key={item.id} item={item} onDismiss={dismiss} closeLabel={closeLabel} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast must be used within a <ToastProvider>");
  }
  return ctx;
}
