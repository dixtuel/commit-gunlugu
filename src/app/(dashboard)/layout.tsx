import "./dashboard.css";

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="dash">
      <aside className="sidebar">
        <div className="brand">
          <span className="mark">CG</span>Commit Günlüğü
        </div>
        <nav>
          <a href="/dashboard">Projeler</a>
          <a href="/onboarding">Repo bağla</a>
          <a href="/billing">Faturalandırma</a>
        </nav>
      </aside>
      <div className="main">{children}</div>
    </div>
  );
}
