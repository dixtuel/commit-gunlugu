export default function GirisPage() {
  return (
    <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center", flexDirection: "column", gap: "1.2rem" }}>
      <h1 style={{ fontSize: "1.6rem" }}>Commit Günlüğü&apos;ne giriş yap</h1>
      <a
        className="btn btnPrimary"
        href="/api/auth/signin/github"
        style={{ background: "var(--accent)", color: "var(--accent-ink)", padding: "0.7rem 1.4rem", borderRadius: 8, textDecoration: "none", fontWeight: 600 }}
      >
        GitHub ile devam et
      </a>
    </div>
  );
}
