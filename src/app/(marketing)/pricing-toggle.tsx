"use client";

import { useState } from "react";

const PLANS = [
  { key: "solo", name: "Solo", monthly: 15, yearly: 12, desc: "Tek ürününe changelog kuracak bağımsız geliştirici.", features: ["1 repo", "Sınırsız ziyaretçi ve AI özet", "Widget + changelog sayfası"], reco: false },
  { key: "agency", name: "Agency", monthly: 29, yearly: 23, desc: "Birden çok müşteriye hizmet veren freelancer ya da ajans.", features: ["5 repo", "Tam beyaz etiket", "Özel alan adı", "Öncelikli AI işleme"], reco: true },
  { key: "studio", name: "Studio", monthly: 35, yearly: 28, desc: "Büyüyen ajans, 10'dan fazla aktif müşteri projesi.", features: ["15 repo dahil, ek repo +$3", "Ekip erişimi", "API erişimi"], reco: false },
] as const;

export function PricingSection() {
  const [yearly, setYearly] = useState(false);

  return (
    <section className="pricing" id="fiyatlar">
      <div className="shell">
        <div className="pricingHead">
          <h2>Fiyatlandırma</h2>
          <p>Proje/repo başına sabit fiyat. Kullanıcı sayınız arttıkça faturanız artmaz.</p>
        </div>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: "0.8rem", margin: "1.8rem 0 0.5rem", fontSize: "0.86rem", color: "var(--ink-soft)" }}>
          <span>aylık</span>
          <button
            role="switch"
            aria-checked={yearly}
            aria-label="Yıllık faturalandırmaya geç"
            onClick={() => setYearly((v) => !v)}
            style={{
              position: "relative", width: 46, height: 26, borderRadius: 999,
              background: yearly ? "var(--accent)" : "var(--line-strong)", border: "none", cursor: "pointer",
            }}
          >
            <span
              style={{
                position: "absolute", top: 3, left: yearly ? 23 : 3, width: 20, height: 20, borderRadius: "50%",
                background: "var(--paper-raised)", transition: "left 0.18s ease",
              }}
            />
          </button>
          <span>
            yıllık{" "}
            <span style={{ fontFamily: "IBM Plex Mono, monospace", fontSize: "0.68rem", color: "var(--amber)", background: "var(--amber-bg)", padding: "0.1rem 0.45rem", borderRadius: 4 }}>
              %20 tasarruf
            </span>
          </span>
        </div>

        <div className="pricingGrid">
          {PLANS.map((plan) => (
            <div key={plan.key} className={`priceCard${plan.reco ? " reco" : ""}`}>
              {plan.reco && (
                <span
                  style={{
                    position: "absolute", top: "-0.68rem", left: "1.4rem", background: "var(--accent)", color: "var(--accent-ink)",
                    fontFamily: "IBM Plex Mono, monospace", fontSize: "0.62rem", letterSpacing: "0.05em", textTransform: "uppercase",
                    padding: "0.22rem 0.6rem", borderRadius: 5,
                  }}
                >
                  en çok tercih edilen
                </span>
              )}
              <span className="planName">{plan.name}</span>
              <p className="planDesc">{plan.desc}</p>
              <div className="planPrice">
                <span>${yearly ? plan.yearly : plan.monthly}</span>
                <span className="per">/ay</span>
              </div>
              <ul className="planList">
                {plan.features.map((f) => (
                  <li key={f}>{f}</li>
                ))}
              </ul>
              <a className={`btn ${plan.reco ? "btnPrimary" : "btnGhost"}`} style={{ justifyContent: "center", marginTop: "0.4rem" }} href="/giris">
                {plan.name} ile başla
              </a>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
