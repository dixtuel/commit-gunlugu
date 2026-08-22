# Mimari

Detaylı pazar/fiyatlandırma analizi için önce yayınlanan plan belgesine bakın (Claude Artifact
olarak paylaşıldı, bu repoda kopyası yok — konuşma geçmişinde link mevcut).

## Bileşenler

```
GitHub (push / PR merge)
        │  webhook
        ▼
Next.js API route  ──►  imza doğrulama, idempotency (WebhookEvent), Project eşleştirme
        │  BullMQ job
        ▼
Worker (ayrı process) ──► AI özetleme (LiteLLM gateway) ──► Entry (status=DRAFT)
        │
        ▼
Dashboard (Next.js, server components) ──► kullanıcı onaylar / düzenler
        │
        ▼
Entry.status = PUBLISHED
        │
        ├──► /api/widget/[widgetKey]  (CORS açık, public) ──► gömülebilir widget script
        └──► /c/[slug]                (public changelog sayfası)
```

## Neden bu ayrımlar

- **Webhook route ↔ worker ayrımı**: GitHub webhook'ları birkaç saniye içinde 2xx bekler; LLM
  çağrısı bunu garanti edemez. Route sadece doğrulama + kuyruğa yazma yapar, ağır iş worker'da.
- **AI çağrısı LiteLLM gateway üzerinden**: `mikoshi-ai-gateway` (LiteLLM proxy) zaten VDS'te
  çalışıyor — model seçimini ve anahtar yönetimini merkezi tutmak için aynı gateway'e
  `AI_API_BASE_URL` ile bağlanılabilir; ayrı bir API anahtarı yönetimi gerekmez.
- **WebhookEvent tablosu**: GitHub aynı delivery'i tekrar gönderebilir (retry). `githubDeliveryId`
  üzerinde unique constraint idempotency'yi garanti eder.
- **Widget vanilla JS, framework'süz**: müşteri sitesine gömülen kod, o sitenin performansını
  etkiler. React/Vue bağımlılığı almadan <15KB hedefi tutturulmalı.

## Dağıtım (öneri)

Mevcut Mikoshi VDS altyapısına (Caddy + Cloudflare Tunnel + Docker Compose) yeni servisler
olarak eklenebilir — bkz. `docs/DEPLOYMENT.md`. Bu repo şimdilik bağımsız tutuluyor; VDS'in
`docker-compose.yml` dosyasına dokunulmadı.

## Yapılmadı / bilinçli olarak MVP dışı

- Ödeme akışı (Stripe) şema alanları var ama webhook/checkout entegrasyonu yazılmadı.
- Çoklu-organizasyon / rol yönetimi (`OrgMember.role`) şemada var, UI'da uygulanmadı.
- E-posta dijesti, çoklu dil çevirisi — plan belgesindeki Faz 4 kapsamı.
