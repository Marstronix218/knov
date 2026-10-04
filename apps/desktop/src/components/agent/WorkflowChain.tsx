import {
  AppWindow,
  ArrowRight,
  BookOpen,
  Bot,
  CalendarDays,
  CirclePlay,
  FileCode2,
  GitPullRequest,
  Globe,
  ListChecks,
  Mail,
  MessageSquare,
  NotebookPen,
  PenTool,
  Search,
  ShieldAlert,
  SquareTerminal,
  Table2,
} from "lucide-react";
import type { WorkflowStep } from "../../types";

export function CategoryIcon({ category, size = 14 }: { category: string; size?: number }) {
  const props = { size, "aria-hidden": true };
  switch (category) {
    case "code": return <FileCode2 {...props} />;
    case "terminal": return <SquareTerminal {...props} />;
    case "code-hosting": return <GitPullRequest {...props} />;
    case "docs": return <BookOpen {...props} />;
    case "search": return <Search {...props} />;
    case "communication": return <MessageSquare {...props} />;
    case "email": return <Mail {...props} />;
    case "notes": return <NotebookPen {...props} />;
    case "spreadsheet": return <Table2 {...props} />;
    case "calendar": return <CalendarDays {...props} />;
    case "video": return <CirclePlay {...props} />;
    case "ai": return <Bot {...props} />;
    case "design": return <PenTool {...props} />;
    case "project": return <ListChecks {...props} />;
    case "sensitive": return <ShieldAlert {...props} />;
    case "browser":
    case "web": return <Globe {...props} />;
    default: return <AppWindow {...props} />;
  }
}

export function WorkflowChain({ steps, highlight }: { steps: WorkflowStep[]; highlight?: number }) {
  return (
    <ol className="workflow-chain" aria-label="Workflow steps">
      {steps.map((step, index) => (
        <li key={`${step.key}-${index}`} className={index === highlight ? "next" : index < (highlight ?? -1) ? "done" : ""}>
          <span className={`chain-step cat-${step.category}`} title={step.title}>
            <CategoryIcon category={step.category} />
            <span>{step.label}</span>
          </span>
          {index < steps.length - 1 && <ArrowRight className="chain-arrow" size={13} aria-hidden="true" />}
        </li>
      ))}
    </ol>
  );
}
