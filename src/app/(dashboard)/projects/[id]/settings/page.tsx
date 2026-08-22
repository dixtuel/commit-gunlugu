import { db } from "@/lib/db";

export default async function ProjectSettingsPage({ params }: { params: { id: string } }) {
  const project = await db.project.findUniqueOrThrow({ where: { id: params.id } });
  const embedSnippet = `<script src="${process.env.WIDGET_CDN_BASE_URL}/${project.widgetKey}.js" async></script>`;

  return (
    <>
      <div className="pageHead">
        <h1>Marka ayarları</h1>
        <p>Widget ve genel changelog sayfanızın görünümü.</p>
      </div>

      <div className="card">
        <div className="formRow">
          <label htmlFor="brandName">Görünen marka adı</label>
          <input id="brandName" name="brandName" defaultValue={project.brandName ?? project.name} />
        </div>
        <div className="formRow">
          <label htmlFor="brandColor">Vurgu rengi</label>
          <input id="brandColor" name="brandColor" type="color" defaultValue={project.brandColor ?? "#3a6b52"} />
        </div>
        <div className="formRow">
          <label htmlFor="customDomain">Özel alan adı</label>
          <input id="customDomain" name="customDomain" placeholder="changelog.musteri.com" defaultValue={project.customDomain ?? ""} />
        </div>
        <button className="btn btnPrimary" type="submit">Kaydet</button>
      </div>

      <div className="card" style={{ marginTop: "1.2rem" }}>
        <strong>Gömme kodu</strong>
        <p style={{ color: "var(--ink-soft)", fontSize: "0.88rem", margin: "0.5rem 0 0.9rem" }}>
          Bu satırı müşterinizin sitesine, kapanış <code>&lt;/body&gt;</code> etiketinden önce ekleyin.
        </p>
        <pre
          style={{
            background: "var(--paper-sunken)", border: "1px solid var(--line)", borderRadius: 8,
            padding: "0.9rem 1rem", fontFamily: "IBM Plex Mono, monospace", fontSize: "0.82rem", overflowX: "auto",
          }}
        >
          {embedSnippet}
        </pre>
      </div>
    </>
  );
}
