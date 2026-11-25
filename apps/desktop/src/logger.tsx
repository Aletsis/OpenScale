import { invoke } from "@tauri-apps/api/core";
import { Component, type ErrorInfo, type ReactNode } from "react";

export type FrontendLogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG";

let handlersInstalled = false;

export function logFrontendEvent(
  level: FrontendLogLevel,
  source: string,
  message: string,
  stack?: string,
  context?: string,
): void {
  void invoke("log_frontend_event", {
    level,
    source,
    message,
    stack: stack ?? null,
    context: context ?? null,
  }).catch(() => {
    // Ignore when running outside Tauri (e.g. Vite browser preview or tests).
  });
}

export function logFrontendError(source: string, err: unknown, context?: string): void {
  if (err instanceof Error) {
    logFrontendEvent("ERROR", source, err.message || err.name, err.stack, context);
  } else {
    logFrontendEvent("ERROR", source, String(err), undefined, context);
  }
}

export function installGlobalErrorHandlers(): void {
  if (handlersInstalled || typeof window === "undefined") {
    return;
  }
  handlersInstalled = true;

  window.addEventListener("error", (event) => {
    const location = event.filename
      ? `${event.filename}:${event.lineno}:${event.colno}`
      : undefined;
    const err = event.error;
    const message = err instanceof Error ? err.message : event.message || "Uncaught window error";
    const stack = err instanceof Error ? err.stack : undefined;
    logFrontendEvent("ERROR", "window.error", message, stack, location);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    if (reason instanceof Error) {
      logFrontendEvent("ERROR", "unhandledrejection", reason.message || reason.name, reason.stack);
    } else {
      logFrontendEvent("ERROR", "unhandledrejection", String(reason));
    }
  });
}

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class AppErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    const combinedStack = [error.stack, info.componentStack]
      .filter((part): part is string => Boolean(part?.trim()))
      .join("\n--- React Component Stack ---\n");
    logFrontendEvent(
      "ERROR",
      "ReactErrorBoundary",
      error.message || error.name,
      combinedStack || undefined,
    );
  }

  override render(): ReactNode {
    if (this.state.error) {
      return (
        <div className="flex min-h-screen items-center justify-center bg-background p-6 text-foreground">
          <div className="w-full max-w-lg space-y-4 rounded-lg border border-destructive/40 bg-card p-6 shadow-sm">
            <h1 className="text-lg font-semibold text-destructive">
              Ocurrió un error inesperado en la interfaz
            </h1>
            <p className="text-sm text-muted-foreground">
              El detalle completo fue guardado automáticamente en el archivo de logs de la
              aplicación.
            </p>
            <pre className="max-h-40 overflow-auto rounded bg-muted p-3 font-mono text-xs text-foreground">
              {this.state.error.message}
            </pre>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => window.location.reload()}
                className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground"
              >
                Recargar aplicación
              </button>
              <button
                type="button"
                onClick={() => void invoke("open_logs_dir").catch(() => {})}
                className="rounded-md border border-border bg-background px-3 py-1.5 text-sm font-medium text-foreground"
              >
                Abrir carpeta de logs
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
