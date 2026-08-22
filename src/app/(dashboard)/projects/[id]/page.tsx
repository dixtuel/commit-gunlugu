import { db } from "@/lib/db";
import { EntryActions } from "./entry-actions";

const CATEGORY_BADGE: Record<string, string> = { NEW: "badgeNew", FIX: "badgeFix", IMPROVEMENT: "badgeImp" };
const CATEGORY_LABEL: Record<string, string> = { NEW: "yeni", FIX: "düzeltme", IMPROVEMENT: "iyileştirme" };

export default async function ProjectPage({ params }: { params: { id: string } }) {
  const project = await db.project.findUniqueOrThrow({ where: { id: params.id } });
  const drafts = await db.entry.findMany({
    where: { projectId: params.id, status: "DRAFT" },
    orderBy: { createdAt: "desc" },
  });

  return (
    <>
      <div className="pageHead">
        <h1>{project.name}</h1>
        <p>{project.githubRepoFullName} · onaylanan girdiler widget&apos;ta anında görünür</p>
      </div>

      {drafts.length === 0 ? (
        <div className="card emptyState">
          <p>Onay bekleyen taslak yok. Yeni bir push veya merge geldiğinde burada görünecek.</p>
        </div>
      ) : (
        drafts.map((entry) => (
          <div className="entryCard" key={entry.id}>
            <div className="meta">
              <span className={`badge ${CATEGORY_BADGE[entry.category]}`}>{CATEGORY_LABEL[entry.category]}</span>
              <span>AI taslağı</span>
            </div>
            <h3>{entry.title}</h3>
            <p>{entry.body}</p>
            <EntryActions projectId={project.id} entryId={entry.id} />
          </div>
        ))
      )}

      <div style={{ marginTop: "2rem" }}>
        <a className="btn btnGhost" href={`/projects/${project.id}/settings`}>Marka ayarları</a>
      </div>
    </>
  );
}
