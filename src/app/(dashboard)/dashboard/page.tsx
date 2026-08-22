import { db } from "@/lib/db";
import { getSessionOrgId } from "@/lib/session";
import { redirect } from "next/navigation";

export default async function DashboardPage() {
  const organizationId = await getSessionOrgId();
  if (!organizationId) redirect("/giris");

  const projects = await db.project.findMany({
    where: { organizationId },
    include: { _count: { select: { entries: { where: { status: "DRAFT" } } as never } } },
  });

  return (
    <>
      <div className="pageHead">
        <h1>Projeleriniz</h1>
        <p>Her proje bir GitHub reposuna karşılık gelir. Onay bekleyen taslaklar burada işaretlenir.</p>
      </div>

      {projects.length === 0 ? (
        <div className="card emptyState">
          <p>Henüz bağlı bir repo yok.</p>
          <a className="btn btnPrimary" href="/onboarding">İlk reponuzu bağlayın</a>
        </div>
      ) : (
        <div className="card">
          {projects.map((project) => (
            <div className="cardRow" key={project.id}>
              <div>
                <strong>{project.name}</strong>
                <div style={{ fontSize: "0.82rem", color: "var(--ink-faint)", fontFamily: "IBM Plex Mono, monospace" }}>
                  {project.githubRepoFullName}
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                {project._count.entries > 0 && (
                  <span className="badge badgeImp">{project._count.entries} onay bekliyor</span>
                )}
                <a className="btn btnGhost" href={`/projects/${project.id}`}>Aç</a>
              </div>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
