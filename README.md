# Commit Günlüğü

GitHub commit ve pull request'lerini okuyup müşteri diline çevrilmiş bir "Yenilikler"
bültenine dönüştüren, siteye tek satır script ile gömülen bir changelog widget'ı ve panosu.

Ajans/freelancer segmentine (birden fazla müşteri projesine beyaz etiketli changelog kuran
geliştiriciler) odaklanır — bkz. `docs/ARCHITECTURE.md`.

> **Durum:** Uçtan uca tasarlanmış bir MVP scaffold'u. Bu ortamda Node.js kurulu olmadığı için
> `npm install` / `next build` burada çalıştırılıp doğrulanmadı — bir sonraki adım gerçek bir
> geliştirme ortamında kurulumu tamamlamak ve derlemeyi doğrulamaktır.

## Yapı

```
src/app/(marketing)   pazarlama sitesi (/) — hero, fiyatlandırma, "nasıl çalışır"
src/app/(dashboard)   panel: proje listesi, taslak onay akışı, marka ayarları, faturalandırma
src/app/c/[slug]      genel changelog sayfası (müşteriye görünen)
src/app/api           GitHub webhook, GitHub App install callback, entry CRUD, widget API
src/worker            BullMQ worker — commit/PR'ı AI ile changelog taslağına çevirir
src/lib               db (Prisma), GitHub App istemcisi, AI özetleme, auth, session
widget/               bağımsız, framework'süz gömülebilir widget script'i (esbuild ile derlenir)
prisma/schema.prisma  veri modeli
docs/                 mimari ve dağıtım notları
```

## Kurulum (bir sonraki adım)

```bash
npm install
cp .env.example .env   # değerleri doldurun
npx prisma migrate dev
npm run dev             # Next.js, :3000
npm run worker          # ayrı terminalde — BullMQ worker
```

## Tasarım kimliği

Pazarlama sitesi ve panel, önceden onaylanan tasarım taslağıyla aynı token sistemini kullanır:
orman yeşili aksan (`--accent: #3a6b52`), Source Serif 4 / Public Sans / IBM Plex Mono üçlüsü,
diff ve commit-log motifleri. Token'lar `src/app/globals.css` içinde tanımlı.
