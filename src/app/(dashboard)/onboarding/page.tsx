export default function OnboardingPage() {
  const githubAppSlug = process.env.NEXT_PUBLIC_GITHUB_APP_SLUG ?? "commit-gunlugu";

  return (
    <>
      <div className="pageHead">
        <h1>Reponuzu bağlayın</h1>
        <p>GitHub App&apos;i kurun, hangi repoların izleneceğini seçin. Webhook kurulumunu biz yaparız.</p>
      </div>
      <div className="card" style={{ textAlign: "center", padding: "3rem 1.5rem" }}>
        <p style={{ color: "var(--ink-soft)", marginBottom: "1.5rem" }}>
          GitHub&apos;a yönlendirileceksiniz. Sadece izlemek istediğiniz repoları seçmeniz yeterli.
        </p>
        <a className="btn btnPrimary" href={`https://github.com/apps/${githubAppSlug}/installations/new`}>
          GitHub ile bağlan
        </a>
      </div>
    </>
  );
}
