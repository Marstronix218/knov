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
  DashboardData,
  Provider,
  RangeKey,
  SettingsData,
  UsageSlice,
} from "./types";

const navigation = [
  { to: "/dashboard", label: "Now", icon: LayoutDashboard },
  { to: "/threads", label: "Threads", icon: Layers3 },
  { to: "/profile", label: "Memory", icon: Brain },
  { to: "/activity", label: "Activity", icon: Activity },
  { to: "/settings", label: "Settings", icon: Settings },
];

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
            <p>Knov observes foreground apps, permitted window titles, and selected browser activity. Raw history stays on this Mac. Only a minimized profile digest and active chat turns go to your chosen AI provider.</p>
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
              {(["openai", "anthropic"] as Provider[]).map((item) => <button className={provider === item ? "selected" : ""} key={item} onClick={() => setProvider(item)}>{item === "openai" ? "OpenAI" : "Anthropic"}</button>)}
            </div>
            <label className="secret-field">API key<input type="password" value={providerKey} onChange={(event) => setProviderKey(event.target.value)} placeholder={provider === "openai" ? "sk-…" : "sk-ant-…"} /></label>
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
          <div className="brand-caption">Personal context</div>
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
  data.recentActivity.forEach((event) => {
    const title = event.topic?.trim() || event.appName;
    groups.set(title, [...(groups.get(title) ?? []), event]);
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

function makeContextBrief(thread: WorkThread): string {
  const evidence = thread.events.slice(0, 4).map((event) => {
    const resource = event.pageTitle || event.windowTitle || event.appName;
    return `- ${resource} (${domainFromUrl(event.url) || event.appName}, ${formatDuration(event.durationSeconds)})`;
  });
  return [
    `Context brief: ${thread.title}`,
    thread.summary,
    `Suggested next move: ${thread.nextMove}`,
    evidence.length ? `Recent evidence:\n${evidence.join("\n")}` : "Recent evidence: no detailed event is available in this range.",
    "Treat this as provisional behavioral context, not confirmed user intent.",
  ].join("\n\n");
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
  const resume = () => {
    const target = selected.events.find((event) => event.url)?.url;
    if (target) {
      window.open(target, "_blank", "noopener,noreferrer");
      setActionMessage("Opened the latest available resource.");
    } else {
      setActionMessage("No reopenable web resource is available. The context brief is ready to use.");
    }
  };
  const copyBrief = async () => {
    await navigator.clipboard.writeText(makeContextBrief(selected));
    setActionMessage("Context brief copied.");
  };
  const prepareAssistant = () => sessionStorage.setItem("knov.active-context-brief", makeContextBrief(selected));

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
            <button className="primary-button large" onClick={resume}>Resume thread <ArrowUpRight size={17} /></button>
            <a className="ghost-button large" href="#/assistant" onClick={prepareAssistant}><MessageSquareText size={17} /> Ask with context</a>
            <button className="ghost-button large" onClick={() => void copyBrief()}><Copy size={17} /> Copy brief</button>
          </div>
          {actionMessage && <p className="action-message" role="status">{actionMessage}</p>}
        </div>
        <EvidenceRail events={selected.events} />
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

function EvidenceRail({ events }: { events: ActivityEvent[] }) {
  return (
    <div className="evidence-rail">
      <div className="evidence-heading"><Eye size={16} /><span>Why this thread?</span><small>Observed locally</small></div>
      {events.length ? events.slice(0, 4).map((event, index) => (
        <article key={event.id}>
          <span className="evidence-index">{String(index + 1).padStart(2, "0")}</span>
          <ActivityLogo event={event} />
          <div><strong>{event.pageTitle || event.windowTitle || event.appName}</strong><small>{domainFromUrl(event.url) || event.appName} · {formatTime(event.startedAt)}</small></div>
          <time>{formatDuration(event.durationSeconds)}</time>
        </article>
      )) : <p className="evidence-empty">This topic is inferred from aggregate signals; no detailed event is available in this range.</p>}
    </div>
  );
}

function ThreadCard({ thread, selected = false, onSelect }: { thread: WorkThread; selected?: boolean; onSelect: () => void }) {
  return (
    <button className={`thread-card${selected ? " selected" : ""}`} onClick={onSelect}>
      <span className="thread-card-top"><span className={`thread-state ${thread.status}`} />{thread.status}<small>{thread.lastActiveAt ? formatTime(thread.lastActiveAt) : "Needs review"}</small></span>
      <strong>{thread.title}</strong>
      <p>{thread.summary}</p>
      <span className="thread-meta"><span>{thread.events.length} signals</span><span>{formatDuration(thread.totalSeconds)}</span><ChevronRight size={15} /></span>
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
  const resource = useResource(() => api.activity(range, query), [range]);
  const filtered = useMemo(
    () => resource.data?.filter((event) => `${event.appName} ${event.windowTitle} ${event.pageTitle} ${event.topic}`.toLowerCase().includes(query.toLowerCase())),
    [resource.data, query],
  );
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
        <ResourceState {...resource}>
          {() => <ActivityList events={filtered ?? []} />}
        </ResourceState>
      </section>
    </div>
  );
}

function ActivityList({ events, compact = false }: { events: ActivityEvent[]; compact?: boolean }) {
  if (!events.length) return <EmptyState title="No matching activity" detail="Try a broader filter or another time range." />;
  return (
    <div className={`activity-list ${compact ? "compact" : ""}`}>
      {events.map((event) => (
        <article className="activity-row" key={event.id}>
          <time>{formatTime(event.startedAt)}</time>
          <div className="timeline-marker" />
          <ActivityLogo event={event} />
          <div className="activity-copy">
            <div><strong>{event.pageTitle || event.windowTitle || event.appName}</strong><span>{event.appName}</span></div>
            <p>{domainFromUrl(event.url) || event.topic || "Application focus"}</p>
          </div>
          <span className={`source-tag ${event.source}`}>{event.source}</span>
          <strong className="duration">{formatDuration(event.durationSeconds)}</strong>
        </article>
      ))}
    </div>
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
    resource.setData(
      editingId
        ? await api.saveCorrection(label, description || undefined, editingId)
        : await api.saveCorrection(label, description || undefined),
    );
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
        description="Inferences remain editable. Anything you tell Knov directly becomes authoritative until you remove it."
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
            <label>What should Knov know?<input required value={label} onChange={(event) => setLabel(event.target.value)} placeholder="I am no longer working on Project X" /></label>
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
  const activeContextBrief = sessionStorage.getItem("knov.active-context-brief");
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "welcome",
      role: "assistant",
      content: activeContextBrief
        ? "Your selected thread is ready. Ask anything and I’ll use its provisional context brief while distinguishing observation from inference."
        : "I’m ready. I’ll use your local memory naturally and distinguish what you told me from what I inferred.",
      createdAt: new Date().toISOString(),
    },
  ]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!draft.trim() || sending) return;
    const userMessage: ChatMessage = { id: crypto.randomUUID(), role: "user", content: draft.trim(), createdAt: new Date().toISOString() };
    const next = [...messages, userMessage];
    setMessages(next);
    setDraft("");
    setSending(true);
    try {
      const providerMessages = activeContextBrief
        ? [...messages, { ...userMessage, content: `${activeContextBrief}\n\nQuestion: ${userMessage.content}` }]
        : next;
      setMessages([...next, await api.chat(providerMessages)]);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="page assistant-page">
      <PageHeader eyebrow="Context-aware assistant" title="Ask without re-explaining" description="Only this conversation and the visible context brief leave your Mac. Raw activity is never attached." actions={<a className="ghost-button" href="#/dashboard">Back to Now</a>} />
      <section className="chat-shell">
        <div className="chat-context"><ShieldCheck size={15} /><span>{activeContextBrief ? "Using your selected thread brief" : "Using your local memory"}</span><small>Raw activity is never attached</small></div>
        {activeContextBrief && <details className="active-context-preview"><summary>Review context being shared</summary><pre>{activeContextBrief}</pre></details>}
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
        <form className="chat-composer" onSubmit={(event) => void submit(event)}>
          <textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
              if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }
            }}
            placeholder="What should I focus on next?"
            rows={2}
          />
          <button className="primary-button" disabled={!draft.trim() || sending}>Send <ChevronRight size={16} /></button>
        </form>
      </section>
    </div>
  );
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
              <p className="status-detail">Profile digests and chat are sent only when needed. OpenAI requests disable optional storage; both providers may retain API data under your account and their current policies.</p>
              <div className="provider-tabs">
                {(["openai", "anthropic"] as Provider[]).map((provider) => (
                  <button className={settings.provider === provider ? "selected" : ""} key={provider} onClick={() => void patch({ provider })}>
                    {provider === "openai" ? "OpenAI" : "Anthropic"}
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
              <SettingsHeading icon={<Eye />} title="Collection" detail="Foreground app, window title, and permitted browser metadata." />
              <Toggle label="Collection active" detail="The Chrome companion follows the Mac state on its next status check." checked={settings.collectionStatus.enabled} onChange={(enabled) => api.setCollectionEnabled(enabled).then(resource.setData)} />
              <Toggle label="Behavioral guidance" detail="Break and focus suggestions; work-continuity guidance stays on." checked={settings.behavioralGuidanceEnabled} onChange={(behavioralGuidanceEnabled) => void patch({ behavioralGuidanceEnabled })} />
              <Toggle label="Launch at login" detail="Resume local collection after you sign in." checked={settings.launchAtLogin} onChange={(launchAtLogin) => void patch({ launchAtLogin })} />
              <div className="permission-row">
                <div><strong>Accessibility permission</strong><p>Required only for active window titles.</p></div>
                <span className={settings.collectionStatus.accessibilityGranted ? "status-ok" : "status-warn"}>{settings.collectionStatus.accessibilityGranted ? "Granted" : "Not granted"}</span>
                {!settings.collectionStatus.accessibilityGranted && <button className="ghost-button" onClick={() => void api.requestAccessibility()}>Open prompt</button>}
              </div>
              {settings.collectionStatus.degradedReasons.map((reason) => <p className="status-detail" key={reason}>{reason}</p>)}
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
                    <p className="status-detail">Reads the last 30 days from selected local Chrome profiles for browsing context and rebuilds your profile. Foreground app time comes from live local collection because Chrome history durations are not reliable screen-time data.</p>
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
              <SettingsHeading icon={<Trash2 />} title="Delete Knov data" detail="Permanently removes activity, profiles, corrections, recommendations, settings, and provider credentials." />
              <button className="danger-button" onClick={() => setConfirmDelete(true)}>Delete everything</button>
            </section>
          </div>
        )}
      </ResourceState>
      {confirmDelete && (
        <Modal title="Delete everything?" onClose={() => setConfirmDelete(false)}>
          <p className="modal-copy">This cannot be undone from within Knov. All app-owned local data and the Keychain credential will be removed.</p>
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
