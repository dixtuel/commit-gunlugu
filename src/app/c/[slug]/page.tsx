import { db } from "@/lib/db";
import { notFound } from "next/navigation";
import "../../(marketing)/marketing.css";

const CATEGORY_LABEL: Record<string, string> = { NEW: "yeni", FIX: "düzeltme", IMPROVEMENT: "iyileştirme" };
const CATEGORY_TAG: Record<string, string> = { NEW: "tagNew", FIX: "tagFix", IMPROVEMENT: "tagImp" };

export default async function PublicChangelogPage({ params }: { params: { slug: string } }) {
  const project = await db.project.findUnique({
    where: { slug: params.slug },
    include: { entries: { where: { status: "PUBLISHED" }, orderBy: { publishedAt: "desc" } } },
  });

  if (!project) notFound();

  return (
    <div className="marketing">
      <header style={{ padding: "3.5rem 0 1rem", borderBottom: "1px solid var(--line)" }}>
        <div className="shell">
          <h1 style={{ fontSize: "2rem", marginBottom: "0.4rem" }}>{project.brandName ?? project.name}</h1>
          <p style={{ color: "var(--ink-soft)" }}>Yenilikler ve güncellemeler</p>
        </div>
      </header>

      <section style={{ padding: "3rem 0" }}>
        <div className="shell" style={{ maxWidth: 680 }}>
          {project.entries.length === 0 ? (
            <p style={{ color: "var(--ink-soft)" }}>Henüz yayınlanmış bir güncelleme yok.</p>
          ) : (
            <div className="commitLog">
              {project.entries.map((entry) => (
                <div className="commit" key={entry.id}>
                  <span className={`outTag ${CATEGORY_TAG[entry.category]}`} style={{ marginBottom: "0.6rem" }}>
                    {CATEGORY_LABEL[entry.category]}
                  </span>
                  <h3>{entry.title}</h3>
                  <p>{entry.body}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      </section>

      {!project.hideBranding && (
        <footer className="site">
          <div className="shell" style={{ justifyContent: "center" }}>
            <a href="https://commit-gunlugu.com">Commit Günlüğü ile çalışır</a>
          </div>
        </footer>
      )}
    </div>
  );
}
