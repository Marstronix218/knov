import { Brain, ChevronRight, LoaderCircle, X } from "lucide-react";
import { ReactNode, useEffect, useRef } from "react";
import type { RangeKey } from "../types";

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow: string;
  title: string;
  description: string;
  actions?: ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        <div className="eyebrow">{eyebrow}</div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {actions && <div className="header-actions">{actions}</div>}
    </header>
  );
}

export function PanelHeader({ title, subtitle, link }: { title: string; subtitle: string; link?: string }) {
  return (
    <div className="panel-header">
      <div><h2>{title}</h2><p>{subtitle}</p></div>
      {link && <a href={link}>View all <ChevronRight size={14} /></a>}
    </div>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
}: {
  options: { value: T; label: string; count?: number }[];
  value: T;
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={value === option.value}
          className={value === option.value ? "selected" : ""}
          onClick={() => onChange(option.value)}
        >
          {option.label}
          {option.count !== undefined && <span className="segmented-count">{option.count}</span>}
        </button>
      ))}
    </div>
  );
}

export function RangePicker({ value, onChange }: { value: RangeKey; onChange: (range: RangeKey) => void }) {
  return (
    <Segmented
      label="Date range"
      value={value}
      onChange={onChange}
      options={[
        { value: "today", label: "Today" },
        { value: "7d", label: "7 days" },
        { value: "30d", label: "30 days" },
      ]}
    />
  );
}

const FOCUSABLE = "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex='-1'])";

export function Modal({
  title,
  onClose,
  children,
  wide = false,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  const dialog = useRef<HTMLElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : undefined;
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialog.current) return;
      const focusable = [...dialog.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("keydown", handleKey);
      previous?.focus();
    };
  }, []);

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        ref={dialog}
        className={`modal${wide ? " wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header"><h2>{title}</h2><button autoFocus aria-label="Close dialog" onClick={onClose}><X size={18} /></button></div>
        {children}
      </section>
    </div>
  );
}

export function ResourceState<T>({
  data,
  error,
  loading,
  children,
}: {
  data?: T;
  error?: string;
  loading: boolean;
  children: (data: T) => ReactNode;
}) {
  if (loading && !data) return <div className="loading-state"><LoaderCircle className="spin" /><span>Loading local context…</span></div>;
  if (error && !data) return <EmptyState title="Couldn’t load this view" detail={error} />;
  return data ? children(data) : <EmptyState title="Nothing here yet" detail="Complete setup to begin collecting local context." />;
}

export function EmptyState({ title, detail, action, icon }: { title: string; detail: string; action?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="empty-state">
      <div>{icon ?? <Brain size={22} />}</div>
      <h3>{title}</h3>
      <p>{detail}</p>
      {action && <div className="empty-action">{action}</div>}
    </div>
  );
}

export function Toggle({
  label,
  detail,
  checked,
  onChange,
  disabled = false,
}: {
  label: string;
  detail: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className={`toggle-row${disabled ? " disabled" : ""}`}>
      <span><strong>{label}</strong><p>{detail}</p></span>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} />
      <i aria-hidden="true" />
    </label>
  );
}

export function SettingsHeading({ icon, title, detail }: { icon: ReactNode; title: string; detail: string }) {
  return <div className="settings-heading"><div>{icon}</div><span><strong>{title}</strong><p>{detail}</p></span></div>;
}

export function Notice({ tone, children }: { tone: "ok" | "error" | "info"; children: ReactNode }) {
  return <p className={`notice ${tone}`} role={tone === "error" ? "alert" : "status"}>{children}</p>;
}

export function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
