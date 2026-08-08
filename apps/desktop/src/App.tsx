import {
  Activity,
  ArrowUpRight,
  BarChart3,
  Bot,
  Brain,
  Check,
  ChevronRight,
  CircleUserRound,
  Clock3,
  Copy,
  Eye,
  FileCode2,
  KeyRound,
  Layers3,
  LayoutDashboard,
  LoaderCircle,
  LockKeyhole,
  MessageSquareText,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { FormEvent, KeyboardEvent as ReactKeyboardEvent, useEffect, useMemo, useState } from "react";
import "./App.css";
import { MarkdownMessage } from "./components/MarkdownMessage";
import { useResource } from "./hooks/useResource";
import { api, isDesktopRuntime } from "./lib/api";
import { domainFromUrl, formatDuration, formatPercentage, formatTime } from "./lib/format";
import type {
  ActivityEvent,
  ChatMessage,
  ContextEconomics,
  DashboardData,
  MemoryRecord,
  Provider,
  RangeKey,
  SettingsData,
  ThreadContext,
  ThreadContextEvent,
  UsageSlice,
} from "./types";

const navigation = [
  { to: "/dashboard", label: "Now", icon: LayoutDashboard },
  { to: "/threads", label: "Threads", icon: Layers3 },
  { to: "/profile", label: "Memory", icon: Brain },
  { to: "/activity", label: "Activity", icon: Activity },
  { to: "/settings", label: "Settings", icon: Settings },
];

const providers: Provider[] = ["openai", "anthropic", "bedrock"];
const ACTIVITY_PAGE_SIZE = 100;

function providerLabel(provider: Provider): string {
  if (provider === "openai") return "OpenAI";
  if (provider === "anthropic") return "Anthropic";
  return "AWS Bedrock";
}

function providerKeyPlaceholder(provider: Provider): string {
  if (provider === "openai") return "sk-…";
  if (provider === "anthropic") return "sk-ant-…";
  return "ABSK…";
}

function App() {
  const [setupComplete, setSetupComplete] = useState(
    () => localStorage.getItem("knov.setup-complete") === "true",
  );
  const route = useHashRoute();

  if (!setupComplete) {
    return (
      <SetupWizard
        onComplete={() => {
          localStorage.setItem("knov.setup-complete", "true");
          setSetupComplete(true);
        }}
      />
    );
  }

  const page = {
    "/dashboard": <DashboardPage />,
    "/threads": <ThreadsPage />,
    "/activity": <ActivityPage />,
    "/profile": <ProfilePage />,
    "/assistant": <AssistantPage />,
    "/settings": <SettingsPage />,
  }[route];

  return (
    <div className="app-shell">
      <Sidebar route={route} />
      <main className="main-stage">
        {!isDesktopRuntime() && <div className="demo-banner">Browser preview · sample data · controls do not change your Mac</div>}
        {page}
      </main>
    </div>
  );
}

const validRoutes = new Set([...navigation.map(({ to }) => to), "/assistant"]);

function routeFromHash(): string {
  const candidate = window.location.hash.slice(1);
  return validRoutes.has(candidate) ? candidate : "/dashboard";
}

function useHashRoute(): string {
  const [route, setRoute] = useState(routeFromHash);

  useEffect(() => {
    const syncRoute = () => setRoute(routeFromHash());
    window.addEventListener("hashchange", syncRoute);
    if (!validRoutes.has(window.location.hash.slice(1))) {
      window.history.replaceState(null, "", "#/dashboard");
    }
    return () => window.removeEventListener("hashchange", syncRoute);
  }, []);

  return route;
}

function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(0);
  const [provider, setProvider] = useState<Provider>("openai");
  const [providerKey, setProviderKey] = useState("");
  const [selectedProfiles, setSelectedProfiles] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const browsers = useResource(() => api.browserProfiles(), []);

  const steps = ["Welcome", "Permissions", "Browser profiles", "AI provider"];

  const finish = async () => {
    setBusy(true);
    setMessage("");
    try {
      if (providerKey.trim()) {
        await api.saveProviderKey(provider, providerKey.trim());
      }
      await api.setBrowserProfiles(selectedProfiles);
      await api.startBootstrap();
      await api.setCollectionEnabled(true);
      onComplete();
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setup-shell">
      <section className="setup-panel">
        <div className="setup-brand"><LogoMark /><strong>Knov</strong></div>
        <div className="setup-progress">
          {steps.map((label, index) => (
            <div className={index <= step ? "active" : ""} key={label}>
              <span>{index < step ? <Check size={12} /> : index + 1}</span>
              <small>{label}</small>
            </div>
          ))}
        </div>

        {step === 0 && (
          <div className="setup-content">
            <div className="setup-icon"><ShieldCheck size={30} /></div>
            <div className="eyebrow">Your context stays yours</div>
            <h1>An assistant that learns from how you actually work.</h1>
            <p>Knov observes foreground apps, permitted window titles, and selected browser activity. Raw history stays on this Mac. When you select a thread, a visible, sanitized detail packet is packed under a token budget for the AI provider.</p>
            <div className="consent-grid">
              <article><LockKeyhole size={18} /><strong>Local raw data</strong><span>SQLite on this Mac, detailed history retained for 30 days.</span></article>
              <article><Eye size={18} /><strong>Visible collection</strong><span>Pause, exclude, inspect, edit, or delete at any time.</span></article>
              <article><KeyRound size={18} /><strong>Your API key</strong><span>Stored in macOS Keychain and sent only to your provider.</span></article>
            </div>
          </div>
        )}

        {step === 1 && (
          <div className="setup-content narrow">
            <div className="setup-icon"><Eye size={30} /></div>
            <div className="eyebrow">macOS permission</div>
            <h1>Allow window titles—only if you want richer context.</h1>
            <p>Accessibility permission lets Knov read the title of the focused window. It does not grant access to keystrokes, document bodies, screenshots, or the clipboard. Without it, app-duration tracking still works.</p>
            <button className="primary-button setup-action" onClick={() => void api.requestAccessibility()}>Open macOS permission prompt</button>
            <span className="setup-skip">You can grant or revoke this later in System Settings.</span>
          </div>
        )}

        {step === 2 && (
          <div className="setup-content">
            <div className="eyebrow">Cold-start context</div>
            <h1>Select browser profiles.</h1>
            <p>Knov can temporarily inspect up to 90 days of selected history to build the first profile. Days 31–90 are deleted after that first profile succeeds.</p>
            <ResourceState {...browsers}>
              {(profiles) => (
                <div className="setup-browser-grid">
                  {profiles.map((profile) => (
                    <label className={selectedProfiles.includes(profile.id) ? "selected" : ""} key={profile.id}>
                      <input
                        type="checkbox"
                        checked={selectedProfiles.includes(profile.id)}
                        onChange={(event) => setSelectedProfiles(event.target.checked ? [...selectedProfiles, profile.id] : selectedProfiles.filter((id) => id !== profile.id))}
                      />
                      <div className="browser-icon">{profile.browser.slice(0, 1).toUpperCase()}</div>
                      <span><strong>{profile.name}</strong><small>{profile.browser} · {profile.support}</small></span>
                      {selectedProfiles.includes(profile.id) && <Check size={16} />}
                    </label>
                  ))}
                </div>
              )}
            </ResourceState>
          </div>
        )}

        {step === 3 && (
          <div className="setup-content narrow">
            <div className="setup-icon"><KeyRound size={30} /></div>
            <div className="eyebrow">Bring your own key</div>
            <h1>Connect an AI provider.</h1>
            <p>Your key is stored in macOS Keychain. Provider calls originate in the native core, never the browser extension or React interface.</p>
            <div className="provider-tabs">
              {providers.map((item) => <button className={provider === item ? "selected" : ""} key={item} onClick={() => setProvider(item)}>{providerLabel(item)}</button>)}
            </div>
            <label className="secret-field">API key<input type="password" value={providerKey} onChange={(event) => setProviderKey(event.target.value)} placeholder={providerKeyPlaceholder(provider)} /></label>
            {message && <p className="error-message">{message}</p>}
          </div>
        )}

        <footer className="setup-footer">
          <button className="ghost-button" disabled={step === 0} onClick={() => setStep((value) => value - 1)}>Back</button>
          <span>{step + 1} of {steps.length}</span>
          {step < steps.length - 1
            ? <button className="primary-button" disabled={step === 2 && selectedProfiles.length === 0} onClick={() => setStep((value) => value + 1)}>Continue <ChevronRight size={15} /></button>
            : <button className="primary-button" disabled={busy || !providerKey.trim() || selectedProfiles.length === 0} onClick={() => void finish()}>{busy ? <LoaderCircle size={15} className="spin" /> : <Sparkles size={15} />} Build my first profile</button>}
        </footer>
      </section>
    </div>
  );
}

function Sidebar({ route }: { route: string }) {
  const { data: settings, setData } = useResource(() => api.settings(), []);
  const enabled = settings?.collectionStatus.enabled ?? false;

  const toggle = async () => {
    const next = await api.setCollectionEnabled(!enabled);
    setData(next);
  };

  return (
    <aside className="sidebar">
      <div className="brand-lockup">
        <LogoMark />
        <div>
          <div className="brand-name">Knov</div>
          <div className="brand-caption">Remembers more. Sends less.</div>
        </div>
      </div>

      <nav className="sidebar-nav" aria-label="Primary navigation">
        {navigation.map(({ to, label, icon: Icon }) => (
          <a key={to} href={`#${to}`} className={`nav-link${route === to ? " active" : ""}`}>
            <Icon size={18} />
            <span>{label}</span>
          </a>
        ))}
      </nav>

      <div className="sidebar-spacer" />

      <div className={`capture-card ${enabled ? "live" : "paused"}`}>
        <div className="capture-status">
          <span className="pulse-dot" />
          <span>{enabled ? "Collection active" : "Collection paused"}</span>
        </div>
        <p>{enabled ? "Activity stays on this Mac." : "No new activity is being stored."}</p>
        <button className="ghost-button full" onClick={() => void toggle()}>
          {enabled ? <Pause size={15} /> : <Play size={15} />}
          {enabled ? "Pause" : "Resume"}
        </button>
      </div>

      <div className="privacy-note">
        <ShieldCheck size={16} />
        <span>Local-first by design</span>
      </div>
    </aside>
  );
}

function LogoMark() {
  return <img className="brand-mark" src="/knov-icon.svg" alt="" aria-hidden="true" />;
}

function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow: string;
  title: string;
  description: string;
  actions?: React.ReactNode;
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

function RangePicker({ value, onChange }: { value: RangeKey; onChange: (range: RangeKey) => void }) {
  return (
    <div className="segmented">
      {(["today", "7d", "30d"] as RangeKey[]).map((range) => (
        <button key={range} className={value === range ? "selected" : ""} onClick={() => onChange(range)}>
          {range === "today" ? "Today" : range === "7d" ? "7 days" : "30 days"}
        </button>
      ))}
    </div>
  );
}

function DashboardPage() {
  const [range, setRange] = useState<RangeKey>("today");
  const resource = useResource(() => api.dashboard(range), [range]);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshMessage, setRefreshMessage] = useState("");

  const refresh = async () => {
    setRefreshing(true);
    setRefreshMessage("Refreshing profile and recommendations…");
    try {
      await api.refreshProfile();
      await resource.reload();
      setRefreshMessage("Profile refreshed.");
    } catch (cause) {
      setRefreshMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <div className="page now-page">
      <PageHeader
        eyebrow="Your working context"
        title="Pick up where you left off."
        description="Knov reconstructs useful work threads from minimal local signals—without screenshots, audio, keystrokes, or page contents."
        actions={
          <>
            <RangePicker value={range} onChange={setRange} />
            <button className="icon-button" aria-label="Refresh profile" title="Refresh profile" disabled={refreshing} onClick={() => void refresh()}>
              <RefreshCw size={17} className={refreshing ? "spin" : ""} />
            </button>
          </>
        }
      />
      {refreshMessage && <p className="refresh-status" role="status">{refreshMessage}</p>}

      <ResourceState {...resource}>
        {(data) => <DashboardContent data={data} />}
      </ResourceState>
    </div>
  );
}

interface WorkThread {
  id: string;
  title: string;
  summary: string;
  nextMove: string;
  events: ActivityEvent[];
  totalSeconds: number;
  lastActiveAt?: string;
  status: "active" | "cooling" | "discovered";
}

function toThreadId(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "") || "untitled";
}

function deriveThreads(data: DashboardData): WorkThread[] {
  const groups = new Map<string, ActivityEvent[]>();
  const softwareDevelopmentTopic = data.activeTopics.find(
    (topic) => topic.name.toLocaleLowerCase() === "software development",
  )?.name;
  data.recentActivity.forEach((event) => {
    const title = event.topic?.trim() || event.appName;
    groups.set(title, [...(groups.get(title) ?? []), event]);
    if (event.source === "editor" && softwareDevelopmentTopic && title !== softwareDevelopmentTopic) {
      groups.set(softwareDevelopmentTopic, [...(groups.get(softwareDevelopmentTopic) ?? []), event]);
    }
  });
  data.activeTopics.forEach((topic) => {
    if (!groups.has(topic.name)) groups.set(topic.name, []);
  });
  const continuity = data.recommendations.find((item) => item.kind === "continuity");

  return [...groups.entries()].map(([title, events], index) => {
    const sortedEvents = [...events].sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt));
    const apps = [...new Set(events.map((event) => event.appName))];
    const status: WorkThread["status"] = index === 0 ? "active" : events.length ? "cooling" : "discovered";
    return {
      id: toThreadId(title),
      title,
      summary: index === 0 && continuity
        ? continuity.body
        : events.length
          ? `${events.length} recent signal${events.length === 1 ? "" : "s"} across ${apps.slice(0, 3).join(", ")}.`
          : "A recurring topic inferred from recent app, title, and domain signals.",
      nextMove: index === 0 && continuity
        ? continuity.title
        : sortedEvents[0]
          ? `Return to ${sortedEvents[0].pageTitle || sortedEvents[0].windowTitle || sortedEvents[0].appName}.`
          : "Review this thread and confirm whether it is still active.",
      events: sortedEvents,
      totalSeconds: events.reduce((sum, event) => sum + event.durationSeconds, 0),
      lastActiveAt: sortedEvents[0]?.startedAt,
      status,
    };
  }).sort((a, b) => (Date.parse(b.lastActiveAt ?? "") || 0) - (Date.parse(a.lastActiveAt ?? "") || 0));
}

function contextResource(event: ActivityEvent): string | undefined {
  if (event.url) {
    try {
      const url = new URL(event.url);
      if ((url.protocol === "http:" || url.protocol === "https:") && url.hostname && !url.username && !url.password) {
        const path = url.pathname === "/" ? "" : url.pathname;
        return `${url.hostname.replace(/^www\./, "")}${path}`.slice(0, 240);
      }
    } catch {
      // Fall through to metadata-only resources.
    }
  }
  return event.source === "editor" ? editorFilePath(event)?.slice(0, 240) : undefined;
}

function reopenableWebUrl(value?: string): string | undefined {
  if (!value) return undefined;
  const candidate = value.trim();
  try {
    const parsed = new URL(candidate);
    if (
      (parsed.protocol !== "http:" && parsed.protocol !== "https:")
      || !parsed.hostname
      || parsed.username
      || parsed.password
    ) {
      return undefined;
    }
    return candidate;
  } catch {
    return undefined;
  }
}

function makeThreadContext(thread: WorkThread): ThreadContext {
  const events: ThreadContextEvent[] = thread.events.slice(0, 100).map((event) => ({
    observedAt: event.startedAt,
    appName: event.appName.slice(0, 100),
    source: event.source,
    title: (event.pageTitle || event.windowTitle)?.trim().slice(0, 300) || undefined,
    resource: contextResource(event),
    searchQuery: event.searchQuery?.trim().slice(0, 300) || undefined,
    observedActiveSeconds: event.source === "history" || event.source === "editor"
      ? undefined
      : Math.max(0, event.durationSeconds),
  }));
  const modifiedFiles = [...new Map(
    thread.events.flatMap((event) => {
      const savedFile = event.source === "editor" ? editorFilePath(event) : undefined;
      return [...(savedFile ? [savedFile] : []), ...(event.modifiedFiles ?? [])];
    }).map((path) => [path.toLocaleLowerCase(), path]),
  ).values()].slice(0, 16);
  return {
    version: 1,
    subject: thread.title,
    signalCount: thread.events.length,
    apps: [...new Set(thread.events.map((event) => event.appName))].slice(0, 12),
    modifiedFiles,
    observedFrom: thread.events[thread.events.length - 1]?.startedAt,
    observedThrough: thread.lastActiveAt,
    events,
  };
}

function makeContextBrief(context: ThreadContext): string {
  return [
    `Context packet: ${context.subject}`,
    `${context.signalCount} locally observed signals${context.apps.length ? ` across ${context.apps.join(", ")}` : ""}.`,
    context.observedFrom && context.observedThrough
      ? `Observed from ${formatContextDateTime(context.observedFrom)} through ${formatContextDateTime(context.observedThrough)}.`
      : "No detailed timing evidence is available in this range.",
    context.modifiedFiles?.length
      ? `Recent modified files (newest first): ${context.modifiedFiles.join(", ")}.`
      : "No modified-file metadata is available for this thread.",
    "Selected evidence candidates (the native core ranks these under the configured token budget):",
    ...context.events.map((event) => [
      `- ${formatContextDateTime(event.observedAt)} | ${event.source} | ${event.appName}`,
      event.title,
      event.resource && `resource=${event.resource}`,
      event.searchQuery && `search=${event.searchQuery}`,
      event.observedActiveSeconds !== undefined && `observed-active=${event.observedActiveSeconds}s`,
    ].filter(Boolean).join(" | ")),
    "The selected thread subject is used for local memory retrieval. Sanitized event details go only to the selected AI provider when you ask with context.",
    "Browser-history duration remains excluded because it is not reliable foreground time. Treat metadata as provisional evidence, not confirmed intent or completion.",
  ].join("\n\n");
}

function loadThreadContext(): ThreadContext | undefined {
  const serialized = sessionStorage.getItem("knov.active-thread-context");
  if (!serialized) return undefined;
  try {
    const value = JSON.parse(serialized) as Partial<ThreadContext>;
    if (value.version !== 1 || !value.subject || !Array.isArray(value.events)) return undefined;
    return value as ThreadContext;
  } catch {
    return undefined;
  }
}

function formatContextDateTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function DashboardContent({ data }: { data: DashboardData }) {
  const threads = useMemo(() => deriveThreads(data), [data]);
  const storedThread = localStorage.getItem("knov.selected-thread");
  const [selectedId, setSelectedId] = useState(storedThread ?? threads[0]?.id);
  const [actionMessage, setActionMessage] = useState("");
  const selected = threads.find((thread) => thread.id === selectedId) ?? threads[0];
  const focusPercent = data.trackedSeconds ? (data.focusedSeconds / data.trackedSeconds) * 100 : 0;

  if (!selected) {
    return <EmptyState title="No work threads yet" detail="Keep collection on while you work. Knov will group recent activity into reviewable threads." />;
  }

  const selectThread = (id: string) => {
    setSelectedId(id);
    localStorage.setItem("knov.selected-thread", id);
    setActionMessage("");
  };
  const resume = async () => {
    const target = selected.events
      .map((event) => reopenableWebUrl(event.url))
      .find((url): url is string => Boolean(url));
    if (target) {
      setActionMessage("Opening the latest available resource…");
      try {
        await api.openResource(target);
        setActionMessage("Opened the latest available resource.");
      } catch {
        setActionMessage("Could not open the latest available resource.");
      }
      return;
    }

    const appName = selected.events
      .map((event) => event.appName.trim())
      .find((name) => {
        const normalized = name.toLocaleLowerCase();
        return Boolean(name) && normalized !== "unknown" && normalized !== "knov";
      });
    if (!appName) {
      setActionMessage("No reopenable resource or local application is available. The context brief is ready to use.");
      return;
    }

    setActionMessage(`Opening ${appName}…`);
    try {
      await api.openApplication(appName);
      setActionMessage(`Opened ${appName}.`);
    } catch {
      setActionMessage(`Could not open ${appName}.`);
    }
  };
  const copyBrief = async () => {
    await navigator.clipboard.writeText(makeContextBrief(makeThreadContext(selected)));
    setActionMessage("Detailed context packet copied.");
  };
  const prepareAssistant = () => sessionStorage.setItem(
    "knov.active-thread-context",
    JSON.stringify(makeThreadContext(selected)),
  );

  return (
    <>
      <section className="now-status" aria-label="Current context status">
        <span><i className="status-light" /> {threads.length} active thread{threads.length === 1 ? "" : "s"}</span>
        <span>{formatDuration(data.trackedSeconds)} observed</span>
        <span>{formatPercentage(focusPercent)} sustained focus</span>
        <span className="local-boundary"><ShieldCheck size={15} /> Detailed activity stays local</span>
      </section>

      <section className="resume-card">
        <div className="resume-orbit" aria-hidden="true"><span /><span /><span /><span /></div>
        <div className="resume-content">
          <div className="resume-kicker"><span className={`thread-state ${selected.status}`} /> Continue where you left off</div>
          <h2>{selected.title}</h2>
          <p>{selected.summary}</p>
          <div className="next-move"><Sparkles size={17} /><span><small>Suggested next move</small><strong>{selected.nextMove}</strong></span></div>
          <div className="resume-actions">
            <button className="primary-button large" onClick={() => void resume()}>Resume thread <ArrowUpRight size={17} /></button>
            <a className="ghost-button large" href="#/assistant" onClick={prepareAssistant}><MessageSquareText size={17} /> Ask with context</a>
            <button className="ghost-button large" onClick={() => void copyBrief()}><Copy size={17} /> Copy brief</button>
          </div>
          {actionMessage && <p className="action-message" role="status">{actionMessage}</p>}
        </div>
        <EvidenceRail events={selected.events} previewEvent={selected.events.find((event) => event.url)} />
      </section>

      <section className="thread-section">
        <div className="section-title-row">
          <div><div className="eyebrow">Your current landscape</div><h2>Active threads</h2></div>
          <a className="section-link" href="#/threads">Explore all <ChevronRight size={15} /></a>
        </div>
        <div className="thread-grid">
          {threads.slice(0, 4).map((thread) => <ThreadCard key={thread.id} thread={thread} selected={thread.id === selected.id} onSelect={() => selectThread(thread.id)} />)}
        </div>
      </section>

      <details className="attention-disclosure">
        <summary><span><BarChart3 size={17} /> Attention details</span><small>Supporting evidence, not a productivity score</small></summary>
        <div className="attention-grid">
          <section className="panel"><PanelHeader title="Application attention" subtitle="Observed foreground usage" /><UsageBars items={data.appUsage} /></section>
          <section className="panel"><PanelHeader title="Web attention" subtitle="Active and imported browser activity" /><DonutSummary items={data.siteUsage} /></section>
          <section className="panel attention-activity"><PanelHeader title="Recent evidence" subtitle="Observed facts from this Mac" link="#/activity" /><ActivityList events={data.recentActivity.slice(0, 4)} compact /></section>
          <section className="panel"><PanelHeader title="Patterns worth reviewing" subtitle="Cautious inferences, not conclusions" /><div className="insight-list">{data.insights.map((insight) => <article className="insight-row" key={insight.id}><div className="insight-metric">{insight.metric}</div><div><h3>{insight.title}</h3><p>{insight.description}</p><span title={insight.evidence}>Evidence available</span></div></article>)}</div></section>
        </div>
      </details>
    </>
  );
}

function PanelHeader({ title, subtitle, link }: { title: string; subtitle: string; link?: string }) {
  return (
    <div className="panel-header">
      <div><h2>{title}</h2><p>{subtitle}</p></div>
      {link && <a href={link}>View all <ChevronRight size={14} /></a>}
    </div>
  );
}

function UsageBars({ items }: { items: UsageSlice[] }) {
  return (
    <div className="usage-list">
      {items.map((item) => (
        <div className="usage-row" key={item.name}>
          <div className="usage-meta">
            <span>{item.name}</span>
            <small>{item.detail}</small>
          </div>
          <div className="usage-track"><span style={{ width: `${item.percentage}%`, backgroundColor: item.color }} /></div>
          <strong>{formatPercentage(item.percentage)}</strong>
          <time>{formatDuration(item.seconds)}</time>
        </div>
      ))}
    </div>
  );
}

function DonutSummary({ items }: { items: UsageSlice[] }) {
  const gradient = items
    .reduce<{ stop: number; segments: string[] }>(
      (acc, item) => {
        const start = acc.stop;
        acc.stop += item.percentage;
        acc.segments.push(`${item.color} ${start}% ${acc.stop}%`);
        return acc;
      },
      { stop: 0, segments: [] },
    )
    .segments.join(", ");
  return (
    <div className="donut-layout">
      <div className="donut" style={{ background: `conic-gradient(${gradient})` }}>
        <div><strong>{items.length - 1}</strong><span>top sites</span></div>
      </div>
      <div className="legend">
        {items.map((item) => (
          <div key={item.name}><i style={{ background: item.color }} /><span>{item.name}</span><strong>{formatPercentage(item.percentage)}</strong></div>
        ))}
      </div>
    </div>
  );
}

function ActivityPreviewCard({ event }: { event: ActivityEvent }) {
  const resource = useResource(() => api.activityPreview(event.url!), [event.url]);
  const [playing, setPlaying] = useState(false);
  const [thumbnailFailed, setThumbnailFailed] = useState(false);
  const [openError, setOpenError] = useState("");
  const title = event.pageTitle || event.windowTitle || event.appName;

  useEffect(() => {
    setPlaying(false);
    setThumbnailFailed(false);
    setOpenError("");
  }, [event.url]);

  const openLink = (
    <a className="preview-link" href={event.url} target="_blank" rel="noreferrer" onClick={(clickEvent) => {
      if (!isDesktopRuntime()) return;
      clickEvent.preventDefault();
      setOpenError("");
      void api.openResource(event.url!).catch(() => setOpenError("Could not open this resource."));
    }}>
      Open resource <ArrowUpRight size={13} />
    </a>
  );

  if (resource.loading) {
    return (
      <section className="activity-preview loading" aria-label="Activity preview">
        <div className="preview-placeholder"><LoaderCircle size={20} className="spin" /></div>
        <div className="preview-copy"><span>Recent resource</span><h3>{title}</h3><small>Loading preview…</small></div>
      </section>
    );
  }

  if (resource.error || !resource.data) {
    return (
      <section className="activity-preview" aria-label="Activity preview">
        <div className="preview-generic"><ActivityLogo event={event} /></div>
        <div className="preview-copy"><span>Recent resource</span><h3>{title}</h3><small>Preview unavailable · the original resource is still available.</small>{openLink}{openError && <small className="preview-error" role="status">{openError}</small>}</div>
      </section>
    );
  }

  const preview = resource.data;
  const previewTitle = preview.title?.trim() || title;
  const playerUrl = preview.embedUrl
    ? `${preview.embedUrl}${preview.embedUrl.includes("?") ? "&" : "?"}autoplay=1`
    : undefined;
  const canPlay = preview.kind === "youtube" && Boolean(playerUrl);
  const hasThumbnail = Boolean(preview.thumbnailDataUrl) && !thumbnailFailed;

  return (
    <section className={`activity-preview ${preview.kind}`} aria-label="Activity preview">
      {playing && playerUrl ? (
        <div className="preview-media">
          <iframe
            src={playerUrl}
            title={previewTitle}
            allow="autoplay; encrypted-media; picture-in-picture; web-share"
            referrerPolicy="strict-origin-when-cross-origin"
            sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"
            allowFullScreen
          />
        </div>
      ) : canPlay ? (
        <button className={`preview-media preview-trigger${hasThumbnail ? "" : " no-image"}`} aria-label={`Play ${previewTitle} preview`} onClick={() => setPlaying(true)}>
          {hasThumbnail && <img src={preview.thumbnailDataUrl} alt={`${previewTitle} preview`} onError={() => setThumbnailFailed(true)} />}
          {!hasThumbnail && <ActivityLogo event={event} />}
          <span><Play size={18} fill="currentColor" /> Play preview</span>
        </button>
      ) : (
        <div className="preview-generic"><ActivityLogo event={event} /></div>
      )}
      <div className="preview-copy">
        <span>{preview.kind === "youtube" ? "Recent video" : "Recent resource"}</span>
        <h3>{previewTitle}</h3>
        <small>{domainFromUrl(event.url) || event.appName} · {formatTime(event.startedAt)}</small>
        <small>{preview.kind === "youtube" ? "YouTube loads only after you press play." : "Live sites are not embedded inside Knov."}</small>
        {openLink}
        {openError && <small className="preview-error" role="status">{openError}</small>}
      </div>
    </section>
  );
}

function EvidenceRail({ events, previewEvent }: { events: ActivityEvent[]; previewEvent?: ActivityEvent }) {
  return (
    <div className="evidence-rail">
      {previewEvent && <ActivityPreviewCard event={previewEvent} />}
      <EditorChangeSummary events={events} />
      <div className="evidence-heading"><Eye size={16} /><span>Why this thread?</span><small>Observed locally</small></div>
      {events.length ? events.slice(0, previewEvent ? 3 : 4).map((event, index) => {
        const appContext = codeActivityContext(event, events);
        return (
          <article key={event.id}>
            <span className="evidence-index">{String(index + 1).padStart(2, "0")}</span>
            <ActivityLogo event={event} />
            <div>
              <strong>{event.pageTitle || event.windowTitle || event.appName}</strong>
              <small title={appContext?.title}>{domainFromUrl(event.url) || event.appName}{appContext ? ` · ${appContext.label}` : ""} · {formatTime(event.startedAt)}</small>
            </div>
            <time>{activityMeasure(event)}</time>
          </article>
        );
      }) : <p className="evidence-empty">This topic is inferred from aggregate signals; no detailed event is available in this range.</p>}
    </div>
  );
}

function ThreadCard({ thread, selected = false, onSelect }: { thread: WorkThread; selected?: boolean; onSelect: () => void }) {
  return (
    <button className={`thread-card${selected ? " selected" : ""}`} onClick={onSelect}>
      <span className="thread-card-top"><span className={`thread-state ${thread.status}`} />{thread.status}<small>{thread.lastActiveAt ? formatTime(thread.lastActiveAt) : "Needs review"}</small></span>
      <strong>{thread.title}</strong>
      <p>{thread.summary}</p>
      <span className="thread-meta"><span>{thread.events.length} signals</span><span>{threadMeasure(thread)}</span><ChevronRight size={15} /></span>
    </button>
  );
}

function ThreadsPage() {
  const [range, setRange] = useState<RangeKey>("7d");
  const resource = useResource(() => api.dashboard(range), [range]);
  const [selectedId, setSelectedId] = useState<string>();
  return (
    <div className="page threads-page">
      <PageHeader eyebrow="Reconstructed work" title="Your threads" description="Knov groups related activity into provisional work streams. Review the evidence before treating an inference as intent." actions={<RangePicker value={range} onChange={setRange} />} />
      <ResourceState {...resource}>
        {(data) => {
          const threads = deriveThreads(data);
          const selected = threads.find((thread) => thread.id === selectedId);
          return (
            <div className="threads-layout">
              <section className="threads-list" aria-label="Work threads">
                {threads.map((thread) => <ThreadCard key={thread.id} thread={thread} selected={thread.id === selectedId} onSelect={() => setSelectedId(thread.id)} />)}
              </section>
              <aside className="thread-detail">
                {selected ? (
                  <>
                    <div className="eyebrow">Thread evidence</div>
                    <h2>{selected.title}</h2>
                    <p>{selected.summary}</p>
                    <div className="next-move"><Sparkles size={17} /><span><small>Suggested next move</small><strong>{selected.nextMove}</strong></span></div>
                    <EvidenceRail events={selected.events} />
                    <a className="primary-button large" href="#/dashboard" onClick={() => localStorage.setItem("knov.selected-thread", selected.id)}>Continue in Now <ArrowUpRight size={17} /></a>
                  </>
                ) : <EmptyState title="Choose a thread" detail="Select a thread to inspect the local evidence and suggested next move." />}
              </aside>
            </div>
          );
        }}
      </ResourceState>
    </div>
  );
}

function ActivityPage() {
  const [range, setRange] = useState<RangeKey>("today");
  const [query, setQuery] = useState("");
  const [visibleCount, setVisibleCount] = useState(ACTIVITY_PAGE_SIZE);
  const resource = useResource(() => api.activity(range, query), [range]);
  const filtered = useMemo(
    () => resource.data?.filter((event) => `${event.appName} ${event.windowTitle} ${event.pageTitle} ${event.searchQuery} ${event.topic}`.toLowerCase().includes(query.toLowerCase())),
    [resource.data, query],
  );
  useEffect(() => setVisibleCount(ACTIVITY_PAGE_SIZE), [range, query]);
  const visibleEvents = filtered?.slice(0, visibleCount) ?? [];
  const hasMore = visibleEvents.length < (filtered?.length ?? 0);
  return (
    <div className="page">
      <PageHeader
        eyebrow="Observed activity"
        title="Your local timeline"
        description="Exactly what Knov recorded—no hidden content, no reconstructed stories."
        actions={<RangePicker value={range} onChange={setRange} />}
      />
      <div className="toolbar">
        <label className="search-field"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Filter apps, pages, or topics" /></label>
        <span className="fact-badge"><LockKeyhole size={14} /> Stored locally for 30 days</span>
      </div>
      <section className="panel activity-panel">
        <EditorChangeSummary events={filtered ?? []} />
        <ResourceState {...resource}>
          {() => (
            <>
              <ActivityList events={visibleEvents} />
              {hasMore && (
                <div className="activity-load-more">
                  <span>Showing {visibleEvents.length} of {filtered?.length ?? 0} events</span>
                  <button className="ghost-button" onClick={() => setVisibleCount((count) => count + ACTIVITY_PAGE_SIZE)}>
                    Show more
                  </button>
                </div>
              )}
            </>
          )}
        </ResourceState>
      </section>
    </div>
  );
}

function ActivityList({ events, compact = false }: { events: ActivityEvent[]; compact?: boolean }) {
  if (!events.length) return <EmptyState title="No matching activity" detail="Try a broader filter or another time range." />;
  return (
    <div className={`activity-list ${compact ? "compact" : ""}`}>
      {events.map((event) => {
        const appContext = codeActivityContext(event, events);
        return (
          <article className="activity-row" key={event.id}>
            <time>{formatTime(event.startedAt)}</time>
            <div className="timeline-marker" />
            <ActivityLogo event={event} />
            <div className="activity-copy">
              <div>
                <strong>{editorFilePath(event) || event.pageTitle || event.windowTitle || event.appName}</strong>
                <span title={appContext?.title}>{event.appName}{appContext ? ` · ${appContext.label}` : ""}</span>
              </div>
              <p>{event.source === "editor" ? `${event.topic || "Editor history"} · File save metadata` : domainFromUrl(event.url) || event.topic || "Application focus"}</p>
            </div>
            <span className={`source-tag ${event.source}`}>{event.source === "editor" ? "file save" : event.source}</span>
            <strong className="duration">{activityMeasure(event)}</strong>
          </article>
        );
      })}
    </div>
  );
}

interface EditorFileChange {
  path: string;
  saves: number;
  lastSavedAt: string;
}

interface CodeActivityContext {
  label: string;
  title: string;
}

const CODE_APP_NAMES = new Set(["code", "visual studio code", "cursor", "cortex code", "xcode"]);
const CODE_FILE_PATTERN = /\.(?:[cm]?[jt]sx?|py|rs|go|java|kt|kts|swift|rb|php|cs|c|cc|cpp|h|hpp|html?|css|scss|sass|less|vue|svelte|sql|sh|bash|zsh|fish|ya?ml|json|toml|xml|graphql|proto)$/i;

function normalizedEditorName(appName: string): string | undefined {
  const normalized = appName.trim().toLocaleLowerCase();
  if (!CODE_APP_NAMES.has(normalized)) return undefined;
  return normalized === "code" || normalized === "visual studio code" ? "visual studio code" : normalized;
}

function codeFileFromWindow(event: ActivityEvent): string | undefined {
  if (!normalizedEditorName(event.appName)) return undefined;
  const candidates = [event.pageTitle, ...(event.windowTitle?.split(/\s+[—–-]\s+/) ?? [])];
  return candidates
    .map((value) => value?.trim().replace(/^[●*]\s*/, ""))
    .find((value): value is string => typeof value === "string" && CODE_FILE_PATTERN.test(value));
}

function codeFileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

function codeActivityContext(event: ActivityEvent, events: ActivityEvent[]): CodeActivityContext | undefined {
  const editorName = normalizedEditorName(event.appName);
  if (!editorName && event.source !== "editor") return undefined;

  const directFile = editorFilePath(event);
  let modifiedFiles = directFile ? [directFile] : [];
  if (!directFile && editorName) {
    const startedAt = Date.parse(event.startedAt);
    const endedAt = startedAt + Math.max(event.durationSeconds, 0) * 1_000;
    modifiedFiles = events.flatMap((candidate) => {
      if (candidate.source !== "editor" || normalizedEditorName(candidate.appName) !== editorName) return [];
      const savedAt = Date.parse(candidate.startedAt);
      return savedAt >= startedAt - 60_000 && savedAt <= endedAt + 60_000
        ? editorFilePath(candidate) ?? []
        : [];
    });
  }

  const uniqueFiles = [...new Map(modifiedFiles.map((path) => [path.toLocaleLowerCase(), path])).values()];
  if (uniqueFiles.length) {
    const names = uniqueFiles.slice(0, 2).map(codeFileName).join(", ");
    const overflow = uniqueFiles.length > 2 ? ` +${uniqueFiles.length - 2} more` : "";
    return {
      label: `Modified ${names}${overflow}`,
      title: `Modified ${uniqueFiles.join(", ")}`,
    };
  }

  const workspaceFiles = [...new Map(
    (event.modifiedFiles ?? []).map((path) => [path.toLocaleLowerCase(), path]),
  ).values()];
  if (workspaceFiles.length) {
    const names = workspaceFiles.slice(0, 2).map(codeFileName).join(", ");
    const overflow = workspaceFiles.length > 2 ? ` +${workspaceFiles.length - 2} more` : "";
    return {
      label: `Changed ${names}${overflow}`,
      title: `Recent workspace changes: ${workspaceFiles.join(", ")}`,
    };
  }

  const visibleFile = codeFileFromWindow(event);
  return visibleFile
    ? { label: `Open ${codeFileName(visibleFile)}`, title: `Visible file: ${visibleFile}; no save detected` }
    : undefined;
}

function editorFilePath(event: ActivityEvent): string | undefined {
  if (event.source !== "editor") return undefined;
  const pageTitle = event.pageTitle?.trim();
  if (pageTitle) return pageTitle;
  const windowTitle = event.windowTitle?.trim();
  if (!windowTitle) return undefined;
  const separator = windowTitle.indexOf(" — ");
  return separator >= 0 ? windowTitle.slice(separator + 3).trim() || undefined : undefined;
}

function summarizeEditorChanges(events: ActivityEvent[]): EditorFileChange[] {
  const files = new Map<string, EditorFileChange>();
  events.forEach((event) => {
    const path = editorFilePath(event);
    if (!path) return;
    const key = path.toLocaleLowerCase();
    const existing = files.get(key);
    if (!existing) {
      files.set(key, { path, saves: 1, lastSavedAt: event.startedAt });
      return;
    }
    existing.saves += 1;
    if (Date.parse(event.startedAt) > Date.parse(existing.lastSavedAt)) existing.lastSavedAt = event.startedAt;
  });
  return [...files.values()].sort((a, b) => Date.parse(b.lastSavedAt) - Date.parse(a.lastSavedAt));
}

function EditorChangeSummary({ events }: { events: ActivityEvent[] }) {
  const files = summarizeEditorChanges(events);
  if (!files.length) return null;
  const saveCount = files.reduce((total, file) => total + file.saves, 0);
  return (
    <section className="editor-change-summary" aria-label="Saved files">
      <div className="editor-change-heading">
        <FileCode2 size={17} />
        <span><strong>Saved files</strong><small>{files.length} file{files.length === 1 ? "" : "s"} · {saveCount} save{saveCount === 1 ? "" : "s"}</small></span>
      </div>
      <div className="editor-file-list">
        {files.slice(0, 6).map((file) => (
          <div className="editor-file-row" key={file.path}>
            <code title={file.path}>{file.path}</code>
            <small>{file.saves} save{file.saves === 1 ? "" : "s"} · {formatTime(file.lastSavedAt)}</small>
          </div>
        ))}
      </div>
      {files.length > 6 && <small className="editor-file-overflow">+{files.length - 6} more file{files.length - 6 === 1 ? "" : "s"}</small>}
      <p>File paths and save times only. Knov does not read code or compute line diffs.</p>
    </section>
  );
}

const activityIconCache = new Map<string, Promise<string | null>>();

function ActivityLogo({ event }: { event: ActivityEvent }) {
  let cacheKey = `app:${event.appName}`;
  if (event.url) {
    try {
      cacheKey = new URL(event.url).origin;
    } catch {
      // An invalid captured URL should still allow the native app icon fallback.
    }
  }
  const [icon, setIcon] = useState<string | null>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    let request = activityIconCache.get(cacheKey);
    if (!request) {
      request = api.activityIcon(event.appName, event.url).catch(() => null);
      activityIconCache.set(cacheKey, request);
    }
    void request.then((resolved) => {
      if (active) setIcon(resolved);
    });
    return () => {
      active = false;
    };
  }, [cacheKey, event.appName, event.url]);

  const fallback = event.appName.slice(0, 1).toUpperCase();
  return (
    <div className="app-token">
      {icon && !failed
        ? <img src={icon} alt="" aria-hidden="true" onError={() => setFailed(true)} />
        : fallback}
    </div>
  );
}

function activityMeasure(event: ActivityEvent): string {
  return event.source === "editor" ? "saved" : formatDuration(event.durationSeconds);
}

function threadMeasure(thread: WorkThread): string {
  return thread.events.length > 0 && thread.events.every((event) => event.source === "editor")
    ? `${thread.events.length} saves`
    : formatDuration(thread.totalSeconds);
}

function ProfilePage() {
  const resource = useResource(() => api.profile(), []);
  const [showForm, setShowForm] = useState(false);
  const [label, setLabel] = useState("");
  const [description, setDescription] = useState("");
  const [editingId, setEditingId] = useState<string>();
  const [summaryDraft, setSummaryDraft] = useState("");
  const [showSummaryForm, setShowSummaryForm] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const profile = editingId
      ? await api.saveCorrection(label, description || undefined, editingId)
      : await api.saveCorrection(label, description || undefined);
    resource.setData(profile);
    setLabel("");
    setDescription("");
    setEditingId(undefined);
    setShowForm(false);
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="Reviewable memory"
        title="What Knov remembers"
        description="Profile facts and authoritative corrections are stored and retrieved locally. Raw activity stays on this Mac."
        actions={<button className="primary-button" onClick={() => { setEditingId(undefined); setLabel(""); setDescription(""); setShowForm(true); }}><Plus size={16} /> Add correction</button>}
      />
      <ResourceState {...resource}>
        {(profile) => (
          <div className="profile-layout">
            <section className="profile-summary">
              <div className="profile-avatar"><CircleUserRound size={34} /></div>
              <div><span>Current understanding</span><p>{profile.summary || "No generated summary is currently saved."}</p></div>
              <div className="inline-actions">
                <button className="ghost-button" onClick={() => { setSummaryDraft(profile.summary); setShowSummaryForm(true); }}>Edit summary</button>
                {profile.summary && <button className="ghost-button" onClick={() => api.saveProfileSummary("").then(resource.setData)}>Clear</button>}
              </div>
            </section>
            <div className="profile-sections">
              {profile.sections.map((section) => (
                <section className="panel profile-section" key={section.id}>
                  <PanelHeader title={section.title} subtitle={`${section.items.length} items`} />
                  <div className="profile-items">
                    {section.items.map((item) => (
                      <article key={item.id}>
                        <div>
                          <strong>{item.label}</strong>
                          {item.description && <p>{item.description}</p>}
                        </div>
                        <span className={`provenance ${item.provenance}`}>
                          {item.provenance === "user" ? <LockKeyhole size={13} /> : item.provenance === "observed" ? <Eye size={13} /> : <Sparkles size={13} />}
                          {item.provenance}
                        </span>
                        {item.provenance === "user" && (
                          <div className="row-actions">
                            <button className="row-action" aria-label="Edit correction" onClick={() => { setEditingId(item.id); setLabel(item.label); setDescription(item.description ?? ""); setShowForm(true); }}><Settings size={15} /></button>
                            <button className="row-action" aria-label="Remove correction" onClick={() => api.removeCorrection(item.id).then(resource.setData)}><Trash2 size={15} /></button>
                          </div>
                        )}
                        {item.provenance === "inferred" && (
                          <button className="row-action" aria-label="Hide inference" onClick={() => api.dismissInference(item.id).then(resource.setData)}>
                            <X size={15} />
                          </button>
                        )}
                      </article>
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </div>
        )}
      </ResourceState>
      {showForm && (
        <Modal title={editingId ? "Edit authoritative correction" : "Add authoritative correction"} onClose={() => setShowForm(false)}>
          <form className="stack-form" onSubmit={(event) => void submit(event)}>
            <label>What should Knov know?<input required value={label} onChange={(event) => setLabel(event.target.value)} placeholder="Privacy is more important to me than adding features" /></label>
            <label>Optional context<textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="This will always override automatic inference." /></label>
            <div className="modal-actions"><button type="button" className="ghost-button" onClick={() => setShowForm(false)}>Cancel</button><button className="primary-button">Save as truth</button></div>
          </form>
        </Modal>
      )}
      {showSummaryForm && (
        <Modal title="Edit profile summary" onClose={() => setShowSummaryForm(false)}>
          <form className="stack-form" onSubmit={(event) => {
            event.preventDefault();
            void api.saveProfileSummary(summaryDraft).then((profile) => {
              resource.setData(profile);
              setShowSummaryForm(false);
            });
          }}>
            <label>Summary<textarea maxLength={600} value={summaryDraft} onChange={(event) => setSummaryDraft(event.target.value)} /></label>
            <div className="modal-actions"><button type="button" className="ghost-button" onClick={() => setShowSummaryForm(false)}>Cancel</button><button className="primary-button">Save summary</button></div>
          </form>
        </Modal>
      )}
    </div>
  );
}

function AssistantPage() {
  const activeThreadContext = loadThreadContext();
  const activeContextBrief = activeThreadContext ? makeContextBrief(activeThreadContext) : undefined;
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "welcome",
      role: "assistant",
      content: activeContextBrief
        ? "Your selected thread is ready. I’ll retrieve only the memories relevant to your question and keep the provisional brief visible."
        : "Ask anything. I’ll retrieve only the approved memories relevant to this request.",
      createdAt: new Date().toISOString(),
    },
  ]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [economics, setEconomics] = useState<ContextEconomics>();
  const [retrievedMemories, setRetrievedMemories] = useState<MemoryRecord[]>([]);
  const [error, setError] = useState("");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!draft.trim() || sending) return;
    const userMessage: ChatMessage = { id: crypto.randomUUID(), role: "user", content: draft.trim(), createdAt: new Date().toISOString() };
    const next = [...messages, userMessage];
    setMessages(next);
    setDraft("");
    setSending(true);
    setError("");
    try {
      const result = await api.chat(next, "optimized", activeThreadContext);
      setMessages([...next, result.message]);
      setEconomics(result.economics);
      setRetrievedMemories(result.retrievedMemories);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="page assistant-page">
      <PageHeader
        eyebrow="Memory-efficient personal AI"
        title="Remembers more. Sends less."
        description="One question shows the full-context baseline, the memories Knov selected, and the resulting token reduction."
        actions={(
          <div className="assistant-header-actions">
            <a className="ghost-button" href="#/dashboard">Back to Now</a>
          </div>
        )}
      />
      <div className="assistant-workspace">
        <section className="chat-shell">
          <div className="chat-context"><ShieldCheck size={15} /><span>Local memory + token-budgeted selected evidence</span><small>Full raw logs stay on this Mac</small></div>
          {activeContextBrief && <details className="active-context-preview"><summary>Review local candidate evidence</summary><pre>{activeContextBrief}</pre></details>}
          <div className="message-list">
            {messages.map((message) => (
              <article className={`message ${message.role}`} key={message.id}>
                <div className="message-avatar">{message.role === "assistant" ? <Bot size={18} /> : <CircleUserRound size={18} />}</div>
                <div className="message-body">
                  {message.role === "assistant" ? (
                    <MarkdownMessage>{message.content}</MarkdownMessage>
                  ) : (
                    <div className="message-content user-content">{message.content}</div>
                  )}
                  <time>{formatTime(message.createdAt)}</time>
                </div>
              </article>
            ))}
            {sending && <article className="message assistant"><div className="message-avatar"><Bot size={18} /></div><div className="typing"><i /><i /><i /></div></article>}
          </div>
          {error && <p className="assistant-error error-message">{error}</p>}
          <form className="chat-composer" onSubmit={(event) => void submit(event)}>
            <textarea
              maxLength={4000}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
                if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
              placeholder="What should I prioritize when building the production version?"
              rows={2}
            />
            <button className="primary-button" disabled={!draft.trim() || sending}>{sending ? <LoaderCircle size={16} className="spin" /> : "Send"} {!sending && <ChevronRight size={16} />}</button>
          </form>
        </section>
        <ContextEconomicsPanel
          economics={economics}
          memories={retrievedMemories}
        />
      </div>
    </div>
  );
}

function ContextEconomicsPanel({
  economics,
  memories,
}: {
  economics?: ContextEconomics;
  memories: MemoryRecord[];
}) {
  return (
    <aside className="economics-panel" aria-label="Context Economics">
      <div className="economics-title"><BarChart3 size={17} /><div><span>Context Economics</span><small>Cost of Intelligence</small></div></div>
      {economics ? (
        <>
          <div className="economics-hero"><strong>{economics.reductionPercent.toFixed(1)}%</strong><span>fewer input tokens</span></div>
          <div className="economics-grid">
            <article><span>Full Context comparison</span><strong>{formatTokenCount(economics.baselineInputTokens)}</strong></article>
            <article className="optimized"><span>Packed prompt</span><strong>{formatTokenCount(economics.optimizedInputTokens)}</strong></article>
            <article><span>Tokens Saved</span><strong>{formatTokenCount(economics.tokensSaved)}</strong></article>
            <article><span>Memories</span><strong>{economics.memoryCount}</strong></article>
            <article><span>Context units</span><strong>{economics.contextUnitsSent}/{economics.contextUnitsConsidered}</strong></article>
            <article><span>Context budget</span><strong>{formatTokenCount(economics.contextEstimatedTokens)}/{formatTokenCount(economics.contextBudgetTokens)}</strong></article>
            <article><span>Cache read</span><strong>{formatTokenCount(economics.cacheReadInputTokens ?? 0)}</strong></article>
            <article><span>Detail level</span><strong>{economics.contextDetailLevel.replace(/-/g, " ")}</strong></article>
          </div>
          <div className="run-meta"><span>{economics.model}</span><span>{economics.latencyMs.toLocaleString()} ms</span><span>{economics.measurementMethod.replace(/_/g, " ")}</span></div>
          <section className="remembered-list">
            <div className="economics-section-title"><Brain size={15} /><span>Knov remembered</span></div>
            {memories.length > 0 ? memories.map((memory) => (
              <article key={memory.id}>
                <div><strong>{memory.text}</strong><small>{memory.memoryType} · {memory.source}</small></div>
                {memory.score !== undefined && <span>{Math.round(memory.score * 100)}%</span>}
              </article>
            )) : <p>No relevant memories were retrieved for this query.</p>}
          </section>
          <details className="context-comparison"><summary>Compare context payloads</summary><div><span>Full Context comparison</span><small>Computed locally and never sent to the AI provider.</small><pre>{economics.baselineContextPreview}</pre></div><div><span>System context (sent)</span><small>The bounded conversation and current question are sent separately.</small><pre>{economics.optimizedContextPreview}</pre></div></details>
          <div className="telemetry-line"><span className="status-ok">{economics.telemetryStatus}</span><small>Query {economics.queryId.slice(0, 8)}</small></div>
        </>
      ) : (
        <div className="economics-empty">
          <strong>Ask one question.</strong>
              <p>Knov will pack the richest relevant context that fits the configured token budget and compare it with the larger baseline.</p>
        </div>
      )}
      <div className="integration-status">
        <article><span className="status-light" /><div><strong>Local memory</strong><small>Profile facts and corrections are retrieved on-device.</small></div></article>
        <article><span className="status-light" /><div><strong>Local economics</strong><small>Aggregate inference metrics stay in Knov's SQLite database.</small></div></article>
      </div>
    </aside>
  );
}

function formatTokenCount(value: number): string {
  return new Intl.NumberFormat("en-US").format(value);
}

function SettingsPage() {
  const resource = useResource(() => api.settings(), []);
  const browsers = useResource(() => api.browserProfiles(), []);
  const pairing = useResource(() => api.pairingInfo(), []);
  const [key, setKey] = useState("");
  const [extensionId, setExtensionId] = useState("");
  const [providerMessage, setProviderMessage] = useState("");
  const [pairingMessage, setPairingMessage] = useState("");
  const [historyImportMessage, setHistoryImportMessage] = useState("");
  const [historyImportError, setHistoryImportError] = useState(false);
  const [historyImporting, setHistoryImporting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const patch = async (settings: Partial<SettingsData>) => {
    resource.setData(await api.saveSettings(settings));
  };

  const reimportHistory = async () => {
    setHistoryImporting(true);
    setHistoryImportError(false);
    setHistoryImportMessage("Re-importing the last 30 days of Chrome history…");
    try {
      await api.reimportChromeHistory();
      setHistoryImportMessage("Chrome durations imported and your profile was refreshed.");
    } catch (cause) {
      setHistoryImportError(true);
      setHistoryImportMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setHistoryImporting(false);
    }
  };

  return (
    <div className="page">
      <PageHeader eyebrow="Control & transparency" title="Settings" description="Every permission, data source, and external request remains visible and reversible." />
      <ResourceState {...resource}>
        {(settings) => (
          <div className="settings-grid">
            <section className="panel settings-card">
              <SettingsHeading icon={<KeyRound />} title="AI provider" detail="Your key goes directly from this Mac to the selected provider." />
              <p className="status-detail">Profile digests and chat are sent only when needed. OpenAI disables optional storage. AWS Bedrock uses model-specific token preflight and eligible prompt caching; provider processing remains governed by your account policy.</p>
              <div className="provider-tabs">
                {providers.map((provider) => (
                  <button className={settings.provider === provider ? "selected" : ""} key={provider} onClick={() => void patch({ provider })}>
                    {providerLabel(provider)}
                  </button>
                ))}
              </div>
              <label className="secret-field">API key<input type="password" value={key} onChange={(event) => setKey(event.target.value)} placeholder={settings.hasProviderKey ? "Key stored in macOS Keychain" : "Paste your key"} /></label>
              <div className="inline-actions">
                <button className="primary-button" disabled={!key} onClick={() => api.saveProviderKey(settings.provider, key).then(() => { setKey(""); setProviderMessage("Saved securely in Keychain."); void resource.reload(); })}>Save key</button>
                <button className="ghost-button" onClick={() => api.testProvider(settings.provider).then(setProviderMessage)}>Test connection</button>
                {settings.hasProviderKey && <button className="ghost-button" onClick={() => api.removeProviderKey(settings.provider).then(() => { setProviderMessage("Key removed from Keychain."); void resource.reload(); })}>Remove key</button>}
              </div>
              {providerMessage && <p className="success-message"><Check size={14} />{providerMessage}</p>}
            </section>

            <section className="panel settings-card">
              <SettingsHeading icon={<Eye />} title="Collection" detail="Foreground app, window title, selected Chrome history, and editor workspace-change metadata." />
              <Toggle label="Collection active" detail="The Chrome companion follows the Mac state on its next status check." checked={settings.collectionStatus.enabled} onChange={(enabled) => api.setCollectionEnabled(enabled).then(resource.setData)} />
              <Toggle label="Behavioral guidance" detail="Break and focus suggestions; work-continuity guidance stays on." checked={settings.behavioralGuidanceEnabled} onChange={(behavioralGuidanceEnabled) => void patch({ behavioralGuidanceEnabled })} />
              <Toggle label="Launch at login" detail="Resume local collection after you sign in." checked={settings.launchAtLogin} onChange={(launchAtLogin) => void patch({ launchAtLogin })} />
              <div className="permission-row">
                <div><strong>Accessibility permission</strong><p>Required only for active window titles.</p></div>
                <span className={settings.collectionStatus.accessibilityGranted ? "status-ok" : "status-warn"}>{settings.collectionStatus.accessibilityGranted ? "Granted" : "Not granted"}</span>
                {!settings.collectionStatus.accessibilityGranted && <button className="ghost-button" onClick={() => void api.requestAccessibility()}>Open prompt</button>}
              </div>
              {settings.collectionStatus.degradedReasons.map((reason) => <p className="status-detail" key={reason}>{reason}</p>)}
              <p className="status-detail">While collection is active, Knov backfills new Chrome visits and reads metadata-only Local History indexes and Git working-tree paths from VS Code, Cursor, and Cortex Code workspaces. It never opens saved code snapshots or source contents.</p>
              {settings.collectionStatus.dataPath && <p className="status-detail">Local database: {settings.collectionStatus.dataPath}</p>}
            </section>

            <section className="panel settings-card full-width">
              <SettingsHeading icon={<BarChart3 />} title="Browser profiles" detail="Chrome is required. Safari and Firefox remain best-effort." />
              <ResourceState {...browsers}>
                {(profiles) => (
                  <>
                    <div className="browser-list">
                      {profiles.map((profile) => (
                        <label key={profile.id}>
                          <input
                            type="checkbox"
                            checked={settings.selectedBrowserProfileIds.includes(profile.id)}
                            onChange={(event) => {
                              const profileIds = event.target.checked
                                ? [...settings.selectedBrowserProfileIds, profile.id]
                                : settings.selectedBrowserProfileIds.filter((id) => id !== profile.id);
                              void api.setBrowserProfiles(profileIds).then(() => patch({ selectedBrowserProfileIds: profileIds }));
                            }}
                          />
                          <div className="browser-icon">{profile.browser.slice(0, 1).toUpperCase()}</div>
                          <div><strong>{profile.name}</strong><p>{profile.browser} · {profile.path}</p><code className="profile-id">ID: {profile.id}</code></div>
                          <span className={`support-badge ${profile.support}`}>{profile.support}</span>
                        </label>
                      ))}
                    </div>
                    <div className="inline-actions">
                      <button
                        className="ghost-button"
                        disabled={historyImporting || settings.selectedBrowserProfileIds.length === 0}
                        onClick={() => void reimportHistory()}
                      >
                        {historyImporting ? <LoaderCircle size={15} className="spin" /> : <Clock3 size={15} />}
                        {historyImporting ? "Re-importing…" : "Re-import Chrome history"}
                      </button>
                    </div>
                    <p className="status-detail">Manual re-import reads the last 30 days and rebuilds your profile. While collection is active, new visits are also backfilled approximately every 30 seconds. Foreground app time still comes from live local collection because Chrome history durations are not reliable screen-time data.</p>
                    {historyImportMessage && (
                      <p className={historyImportError ? "error-message" : "success-message"}>
                        {!historyImportError && <Check size={14} />}
                        {historyImportMessage}
                      </p>
                    )}
                  </>
                )}
              </ResourceState>
            </section>

            <section className="panel settings-card">
              <SettingsHeading icon={<Settings />} title="Chrome companion pairing" detail="Install the unpacked extension in each approved Chrome profile, then register its extension ID." />
              <ResourceState {...pairing}>
                {(info) => (
                  <div className="stack-form">
                    <label>Native host<input readOnly value={info.nativeHost} /></label>
                    <label>Pairing token<input readOnly type="password" value={info.pairingToken} /></label>
                    <label>Chrome extension ID<input value={extensionId} onChange={(event) => setExtensionId(event.target.value.trim())} placeholder="32 characters, a–p" /></label>
                    <button className="primary-button" disabled={extensionId.length !== 32} onClick={() => api.installNativeHost(extensionId).then((path) => setPairingMessage(`Installed manifest: ${path}`))}>Register native host</button>
                    {pairingMessage && <p className="success-message"><Check size={14} />{pairingMessage}</p>}
                  </div>
                )}
              </ResourceState>
            </section>

            <section className="panel settings-card">
              <SettingsHeading icon={<ShieldCheck />} title="Exclusions" detail="Excluded applications and domains are dropped locally before they can affect your profile." />
              <ExclusionEditor
                settings={settings}
                onSave={(excludedApps, excludedDomains) => patch({ excludedApps, excludedDomains })}
              />
            </section>

            <section className="panel settings-card full-width danger-card">
              <SettingsHeading icon={<Trash2 />} title="Delete local Knov data" detail="Permanently removes local activity, profiles, corrections, recommendations, telemetry, settings, and provider credentials." />
              <button className="danger-button" onClick={() => setConfirmDelete(true)}>Delete everything</button>
            </section>
          </div>
        )}
      </ResourceState>
      {confirmDelete && (
        <Modal title="Delete everything?" onClose={() => setConfirmDelete(false)}>
          <p className="modal-copy">This cannot be undone from within Knov. Local app data and provider credentials stored in Keychain will be removed.</p>
          <div className="modal-actions"><button className="ghost-button" onClick={() => setConfirmDelete(false)}>Cancel</button><button className="danger-button" onClick={() => api.deleteAllData().then(() => {
            localStorage.removeItem("knov.setup-complete");
            window.location.hash = "";
            window.location.reload();
          })}>Delete permanently</button></div>
        </Modal>
      )}
    </div>
  );
}

function ExclusionEditor({
  settings,
  onSave,
}: {
  settings: SettingsData;
  onSave: (apps: string[], domains: string[]) => Promise<void>;
}) {
  const [apps, setApps] = useState(settings.excludedApps.join(", "));
  const [domains, setDomains] = useState(settings.excludedDomains.join(", "));
  const split = (value: string) => value.split(",").map((item) => item.trim()).filter(Boolean);
  return (
    <div className="stack-form">
      <label>Applications<input value={apps} onChange={(event) => setApps(event.target.value)} placeholder="Mail, Messages" /></label>
      <label>Domains<input value={domains} onChange={(event) => setDomains(event.target.value)} placeholder="bank.example, health.example" /></label>
      <button className="ghost-button" onClick={() => void onSave(split(apps), split(domains))}>Save exclusions</button>
    </div>
  );
}

function SettingsHeading({ icon, title, detail }: { icon: React.ReactNode; title: string; detail: string }) {
  return <div className="settings-heading"><div>{icon}</div><span><strong>{title}</strong><p>{detail}</p></span></div>;
}

function Toggle({ label, detail, checked, onChange }: { label: string; detail: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="toggle-row">
      <span><strong>{label}</strong><p>{detail}</p></span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <i />
    </label>
  );
}

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal" role="dialog" aria-modal="true" aria-label={title} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><h2>{title}</h2><button autoFocus aria-label="Close dialog" onClick={onClose}><X size={18} /></button></div>
        {children}
      </section>
    </div>
  );
}

function ResourceState<T>({
  data,
  error,
  loading,
  children,
}: {
  data?: T;
  error?: string;
  loading: boolean;
  children: (data: T) => React.ReactNode;
}) {
  if (loading && !data) return <div className="loading-state"><LoaderCircle className="spin" /><span>Loading local context…</span></div>;
  if (error && !data) return <EmptyState title="Couldn’t load this view" detail={error} />;
  return data ? children(data) : <EmptyState title="Nothing here yet" detail="Complete setup to begin collecting local context." />;
}

function EmptyState({ title, detail }: { title: string; detail: string }) {
  return <div className="empty-state"><div><Brain size={22} /></div><h3>{title}</h3><p>{detail}</p></div>;
}

export default App;
