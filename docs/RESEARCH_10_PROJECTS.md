# Açık Kaynak Commit ve Changelog Projeleri İnceleme ve Güvenlik Raporu

Bu rapor, kullanıcının talebi doğrultusunda GitHub üzerindeki **10 popüler açık kaynak commit/changelog/release-notları projesinin** mimarilerini, kaynak kodlarını, konfigürasyonlarını ve geçmiş güvenlik açıklarını (CVE / Security Advisories) detaylıca inceleyerek derlenmiştir.

---

## 1. İncelenen 10 Proje Özeti

| # | Proje Adı | Yıldız | Dil / Çalışma Zamanı | Temel Sorumluluk / Mimari Model |
| :- | :--- | :--- | :--- | :--- |
| 1 | **[orhun/git-cliff](https://github.com/orhun/git-cliff)** | 12.230 ⭐ | Rust | Conventional Commits ayrıştırıcı, Tera şablon motoru, regex tabanlı commit dönüştürücü CLI/kütüphane |
| 2 | **[semantic-release/semantic-release](https://github.com/semantic-release/semantic-release)** | 24.035 ⭐ | JavaScript / Node.js | Tam otomatik paket sürümleme, commit analizi, GitHub Release ve npm dağıtım motoru |
| 3 | **[goreleaser/goreleaser](https://github.com/goreleaser/goreleaser)** | 16.031 ⭐ | Go | Çok platformlu Go derleme, imzalama, artifact paketleme ve otomatik changelog üretimi |
| 4 | **[changesets/changesets](https://github.com/changesets/changesets)** | 12.389 ⭐ | TypeScript / Node.js | Monorepo odaklı sürümleme, PR tabanlı editoryal değişiklik kayıtları (`.changeset/*.md`) |
| 5 | **[conventional-changelog/conventional-changelog](https://github.com/conventional-changelog/conventional-changelog)** | 8.508 ⭐ | TypeScript / Node.js | Conventional Commits ekosisteminin çekirdek ayrıştırıcısı ve changelog üreticisi |
| 6 | **[googleapis/release-please](https://github.com/googleapis/release-please)** | 7.489 ⭐ | TypeScript / GitHub Action | Google standartlarında otomatik Release PR açma, manifest tabanlı kütüphane sürümleme |
| 7 | **[release-drafter/release-drafter](https://github.com/release-drafter/release-drafter)** | 3.938 ⭐ | TypeScript / GitHub Action | PR etiketleri ve commit mesajlarına göre GitHub Release taslaklarını otomatik güncelleyen motor |
| 8 | **[cookpete/auto-changelog](https://github.com/cookpete/auto-changelog)** | 1.398 ⭐ | JavaScript / Handlebars | Git loglarını Handlebars şablonlarıyla HTML/Markdown sürüm günlüğüne dönüştüren araç |
| 9 | **[mikepenz/release-changelog-builder-action](https://github.com/mikepenz/release-changelog-builder-action)** | 866 ⭐ | TypeScript / GitHub Action | İki git tag veya commit arasındaki PR/commit farklarını regex ve kategorilerle derleyen CI aracı |
| 10 | **[stonemaster/github-release-generator](https://github.com/stonemaster/github-release-generator)** | ~1 ⭐ | Rust | GitHub REST API kullanarak commit ve issue'lardan sürüm notu derleyen Rust aracı |

---

## 2. Güvenlik Açıkları, Riskler ve Alınan Karşı Önlemler

GitHub Security Advisories veri tabanında bu projeler üzerinde tespit edilen kritik açıklar ve Commit Günlüğü'nde uygulanan kesin koruma katmanları:

### 2.1. Git CLI Argüman Enjeksiyonu (CVE-2025-59433 - `conventional-changelog`)
- **Açık Detayı:** `conventional-changelog` ve `@conventional-changelog/git-client` paketlerinde, commit referansları ve kullanıcı girdileri doğrudan yerel `git` komut satırı argümanlarına (`git log ...`) iliştirildiğinde, kötü niyetli branch adları veya commit hash'leri (`--output=/tmp/...`) üzerinden sistemde komut enjeksiyonu yapılabiliyordu.
- **Commit Günlüğü'ndeki Çözüm:** Sistemimiz sunucuda `git` CLI çalıştırmaz! GitHub'dan güvenli Webhook JSON payload'ını HTTP üzerinden alır ve bellek içinde Rust veri modellerine deserialize eder. Kabuk (shell) veya argüman geçişi yoktur.

### 2.2. Loglarda ve Hata Ayıklamada Token Sızıntısı (CVE-2024-23840 - `goreleaser`, CVE-2022-31051 - `semantic-release`)
- **Açık Detayı:** `goreleaser --debug` modu veya `semantic-release` hata yakalayıcıları, API çağrıları veya CI logları sırasında ortam değişkenlerindeki (`GITHUB_TOKEN`, `NPM_TOKEN`, webhook secret) gizli anahtarları log dosyalarına düz metin olarak basıyordu.
- **Commit Günlüğü'ndeki Çözüm:** `tracing` yapılandırmamızda secret içeren alanlar `#[serde(skip_serializing)]` ve `#[tracing::instrument(skip(...))]` ile maskelenmiştir. Loglarda webhook secret, parola veya oturum token'ları asla yer almaz.

### 2.3. URL Encode Edilen Özel Karakterlerde Gizli Veri İfşası (CVE-2020-26226 - `semantic-release`)
- **Açık Detayı:** Git remote URL'lerindeki kullanıcı adı/şifre çiftleri URI encode edildiğinde regex temizleyiciler tarafından yakalanamayıp kamuya açık changelog metinlerine sızabiliyordu.
- **Commit Günlüğü'ndeki Çözüm:** `src/sanitizer/mod.rs` modülümüz, e-posta adreslerini ve token paternlerini kapıda (`subtle` ve regex ile) temizler; `author.email` veritabanına veya kamuya açık API'ye kesinlikle kaydedilmez.

### 2.4. Webhook Tekrarlama ve Sahtecilik Saldırıları (Replay Attacks)
- **Risk:** Ağ trafiğini dinleyen bir saldırgan, daha önce GitHub tarafından gönderilmiş geçerli bir webhook imzasını (`X-Hub-Signature-256`) kaydedip sunucuya defalarca yeniden gönderebilir.
- **Commit Günlüğü'ndeki Çözüm:** `webhook_events` tablosunda `github_delivery_id` alanı `UNIQUE` olarak tanımlıdır (`ON CONFLICT DO NOTHING`). Aynı delivery ID ile gelen tekrarlı istekler sıfır işlemle yutulur. Ayrıca imza karşılaştırması `subtle::ConstantTimeEq` ile sabit zamanlı yapılır.

---

## 3. Mimari ve Konfigürasyon Tasarımı Çıkarımları

1. **Şablonlama:** `git-cliff` Tera şablonlarını, `auto-changelog` ise Handlebars kullanır. Commit Günlüğü, Rust ekosisteminde Jinja2 standardını getiren, sıfır bağımlılıklı ve ultra hafif **Minijinja 2** motorunu benimsemiştir.
2. **Kategori Ayrıştırma:** Hem `git-cliff` hem `release-drafter`, Conventional Commits kurallarını (`feat:`, `fix:`, `chore:`, `breaking:`) temel alır. Commit Günlüğü hem bu kural motorunu deterministik olarak içerir hem de üstüne **NVIDIA NIM & Mikoshi AI Gateway** entegrasyonuyla editoryal akıcılık ekler.
3. **Monorepo ve Bağımsızlık:** `changesets` felsefesinde olduğu gibi, değişiklik notları koddan bağımsız bir yaşam döngüsüne sahiptir. Kullanıcı yönetim panelinden taslakları (`DRAFT`) yayına alabilir (`PUBLISHED`) veya silebilir.
