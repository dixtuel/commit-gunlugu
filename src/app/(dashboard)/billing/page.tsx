import { db } from "@/lib/db";
import { getSessionOrgId } from "@/lib/session";
import { redirect } from "next/navigation";

const PLAN_LABELS: Record<string, string> = { SOLO: "Solo · $15/ay", AGENCY: "Agency · $29/ay", STUDIO: "Studio · $35/ay" };

export default async function BillingPage() {
  const organizationId = await getSessionOrgId();
  if (!organizationId) redirect("/giris");

  const org = await db.organization.findUniqueOrThrow({ where: { id: organizationId } });

  return (
    <>
      <div className="pageHead">
        <h1>Faturalandırma</h1>
        <p>Mevcut planınız ve fatura bilgileriniz.</p>
      </div>
      <div className="card">
        <div className="cardRow">
          <div>
            <strong>Mevcut plan</strong>
            <div style={{ fontSize: "0.9rem", color: "var(--ink-soft)" }}>{PLAN_LABELS[org.plan]}</div>
          </div>
          <a className="btn btnGhost" href="/#fiyatlar">Planı değiştir</a>
        </div>
        <div className="cardRow">
          <div>
            <strong>Fatura e-postası</strong>
            <div style={{ fontSize: "0.9rem", color: "var(--ink-soft)" }}>{org.billingEmail ?? "tanımlı değil"}</div>
          </div>
        </div>
      </div>
    </>
  );
}
