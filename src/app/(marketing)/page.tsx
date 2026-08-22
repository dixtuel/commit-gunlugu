import "./marketing.css";
import { PricingSection } from "./pricing-toggle";

export default function MarketingPage() {
  return (
    <div className="marketing">
      <nav className="top">
        <div className="shell">
          <div className="brand">
            <span className="mark">CG</span>Commit Günlüğü
          </div>
          <div className="navLinks">
            <a href="#nasil-calisir">Nasıl çalışır</a>
            <a href="#onizleme">Önizleme</a>
            <a href="#fiyatlar">Fiyatlar</a>
          </div>
          <div className="navCta">
            <a className="btn btnGhost" href="/giris">Giriş yap</a>
            <a className="btn btnPrimary" href="#fiyatlar">Ücretsiz dene</a>
          </div>
        </div>
      </nav>

      <header className="hero">
        <div className="shell">
          <div className="eyebrow"><span className="dot" /> GitHub App · 2 dakikada kurulum</div>
          <h1 className="headline">
            Commit&apos;leriniz zaten
            <br />
            her şeyi anlatıyor. <em>Onu tercüme ediyoruz.</em>
          </h1>
          <p className="subhead">
            Commit Günlüğü, GitHub reponuzdaki commit ve pull request&apos;leri okur, müşterinizin anlayacağı bir
            &quot;Yenilikler&quot; bültenine çevirir ve sitenize tek satırla gömdüğünüz bir widget&apos;ta yayınlar.
            Elle yazmak yok.
          </p>
          <div className="heroActions">
            <a className="btn btnPrimary" href="#fiyatlar">14 gün ücretsiz dene</a>
            <a className="btn btnGhost" href="#onizleme">Widget&apos;ı gör</a>
          </div>
          <div className="heroNote">kredi kartı gerekmez · sınırsız ziyaretçi · sınırsız AI özet</div>

          <div className="transform">
            <div className="panel panelRaw">
              <div className="panelHead">
                <span className="dot3"><span /><span /><span /></span> git log --oneline
              </div>
              <div className="rawBody">
                <div className="rawLine"><span className="hash">a3f91c</span> fix: race condition in webhook queue retry</div>
                <div className="rawLine"><span className="hash">9d02be</span> feat(billing): add prorated plan upgrades</div>
                <div className="rawLine"><span className="hash">1c774a</span> perf: cache repo tree lookups, cut API calls 60%</div>
                <div className="rawLine"><span className="hash">e58b21</span> fix: timezone offset in digest scheduler</div>
              </div>
            </div>
            <div className="arrowCol" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
                <path d="M4 12h15M13 6l6 6-6 6" />
              </svg>
            </div>
            <div className="panel panelOut">
              <div className="panelHead">Yenilikler · bugün</div>
              <div className="outBody">
                <div className="outItem"><span className="outTag tagNew">yeni</span><span className="outText">Artık planınızı yükseltirken kalan günler için orantılı ücretlendirme yapıyoruz.</span></div>
                <div className="outItem"><span className="outTag tagImp">iyileştirme</span><span className="outText">Panonuz artık daha hızlı açılıyor — arka planda önbellekleme iyileştirmesi yaptık.</span></div>
                <div className="outItem"><span className="outTag tagFix">düzeltme</span><span className="outText">Bildirim zamanlamasındaki saat dilimi hatası giderildi.</span></div>
              </div>
            </div>
          </div>
        </div>
      </header>

      <section className="notfor">
        <div className="shell twoCol">
          <div>
            <h2>Geri bildirim panosu değil. Anket aracı değil. Sadece changelog.</h2>
            <p className="lede">
              Piyasadaki araçların çoğu changelog&apos;u; oylama, NPS anketi ve destek masasıyla birlikte satıyor.
              Bunun bedelini hem fiyatta hem karmaşıklıkta ödüyorsunuz. Biz tek bir şeyi iyi yapıyoruz.
            </p>
          </div>
          <div className="diffList">
            <div className="diffRow rm"><span className="sym">−</span><span className="t">Oylama panosu, NPS anketi, destek masası, çoklu modül karmaşası</span></div>
            <div className="diffRow rm"><span className="sym">−</span><span className="t">Aktif kullanıcı sayısına göre büyüyen fatura</span></div>
            <div className="diffRow add"><span className="sym">+</span><span className="t">Repoyu bağla, AI taslağı hazırlar, sen onaylarsın</span></div>
            <div className="diffRow add"><span className="sym">+</span><span className="t">Proje başına sabit fiyat, sınırsız ziyaretçi</span></div>
          </div>
        </div>
      </section>

      <section className="how" id="nasil-calisir">
        <div className="shell">
          <div className="howHead">
            <h2>Kurulumdan yayına, üç adım</h2>
            <p>Sırayla ilerler: önce bağlantı, sonra taslak, en son onay. Aradaki hiçbir adımda elle yazı yazmazsınız.</p>
          </div>
          <div className="commitLog">
            <div className="commit">
              <span className="commitHash">adım 01</span>
              <h3>Reponuzu bağlayın</h3>
              <p>GitHub App ile tek tıkla yetkilendirin, hangi repoların izleneceğini seçin. Webhook&apos;lar otomatik kurulur.</p>
            </div>
            <div className="commit">
              <span className="commitHash">adım 02</span>
              <h3>Taslak kendiliğinden hazırlanır</h3>
              <p>Her merge sonrası commit mesajları ve PR açıklamaları okunur; jargon arındırılmış, kategorilenmiş bir taslak panoda belirir.</p>
            </div>
            <div className="commit">
              <span className="commitHash">adım 03</span>
              <h3>Gözden geçirin, yayınlayın</h3>
              <p>Metni düzenleyin veya olduğu gibi onaylayın. Yayınlandığı an widget ve changelog sayfanız güncellenir.</p>
            </div>
          </div>
        </div>
      </section>

      <PricingSection />

      <section className="finalcta">
        <div className="shell">
          <h2>İlk changelog&apos;unuz on dakika sonra yayında.</h2>
          <p>Repoyu bağlayın, taslağı görün, onaylayın. Kart bilgisi istemiyoruz.</p>
          <a className="btn btnPrimary" href="#fiyatlar">14 gün ücretsiz dene</a>
        </div>
      </section>

      <footer className="site">
        <div className="shell">
          <span>© 2026 Commit Günlüğü</span>
          <span>durum: MVP geliştirme</span>
        </div>
      </footer>
    </div>
  );
}
