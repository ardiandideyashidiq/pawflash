import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
  componentStack: string | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null, componentStack: null };

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("pawflash render error", error, info);
    this.setState({ componentStack: info.componentStack ?? null });
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-dvh w-dvw flex-col items-center justify-center gap-4 bg-background p-8 text-foreground">
          <h1 className="text-lg font-semibold text-error">Something went wrong</h1>
          <pre className="max-h-64 max-w-3xl overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted p-4 font-mono text-xs text-error">
            {this.state.error.message}
            {"\n\n"}
            {this.state.error.stack}
            {this.state.componentStack ? `\n\n${this.state.componentStack}` : ""}
          </pre>
          <button
            type="button"
            className="rounded-md border border-border bg-card px-4 py-2 text-sm hover:bg-accent-soft/70"
            onClick={() => window.location.reload()}
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
