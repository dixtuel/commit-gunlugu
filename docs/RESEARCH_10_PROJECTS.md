# Commit ve changelog araçları: kaynak kod incelemesi

**Araştırma tarihi:** 2026-09-26 · **Amaç:** Git geçmişini, Conventional Commits mesajlarını, PR başlıklarını veya insan tarafından yazılan değişiklik kayıtlarını daha anlaşılır sürüm notlarına dönüştüren açık kaynaklı yaklaşımları karşılaştırmak.

## Yöntem ve kapsam

13 public GitHub deposu `/opt/commit-gunlugu-research/repos/` altına `--depth 1` ile klonlandı (toplam 92 MB). README, lisans dosyası, örnek konfigürasyonlar ve konuyla ilgili kod/dokümanlar incelendi. GitHub API'den yıldız, ana dil, lisans, arşiv durumu ve son push tarihi alındı; yıldız sayıları zamanla değişir. Her klonun o anki `HEAD` özeti aşağıdaki tabloda kayıtlıdır.

On depoda araştırma tarihindeki yıldız sayısı 1.000'in üzerindeydi. `git-chglog` bu gruba girse de arşivlenmiştir; aktif ürün adayı olarak değerlendirilmemelidir. Diğer projeler güncel depolardır. Liste hem commit geçmişini doğrudan ayrıştıranları hem de daha okunur not üretmek için PR/insan girdisi kullanan komşu yaklaşımları içerir. Bunların tümü yapay zekâ ile serbest metni yeniden yazmaz.

## İncelenen depolar

| Depo | Yıldız | Dil / lisans | Son push (UTC) | Girdi → çıktı ve kayda değer fikir | Klon `HEAD` |
|---|---:|---|---|---|---|
| [orhun/git-cliff](https://github.com/orhun/git-cliff) | 12.268 | Rust · MIT/Apache-2.0 | 2026-09-18 | Yerel Git geçmişi → regex/Conventional Commit ayrıştırma, gruplama ve özelleştirilebilir Tera changelog şablonu. Tag ve branch kapsamı, path filtresi seçenekleri var. | `60e0be9` |
| [conventional-changelog/conventional-changelog](https://github.com/conventional-changelog/conventional-changelog) | 8.511 | TypeScript · ISC | 2026-09-25 | Git metadatası ve Conventional Commits → preset tabanlı CHANGELOG; parser, writer ve bump hesaplaması ayrıştırılabilir paketler halinde. | `f90c80e` |
| [semantic-release/semantic-release](https://github.com/semantic-release/semantic-release) | 24.068 | JavaScript · MIT | 2026-09-26 | Conventional Commits → etki/sürüm analizi, release notes ve eklenti zinciriyle otomatik yayın. Tam ürün değil, CI/CD sürümleme otomasyonu. | `e8c2436` |
| [commitizen-tools/commitizen](https://github.com/commitizen-tools/commitizen) | 3.514 | Python · MIT | 2026-09-25 | İnteraktif ve doğrulamalı commit yazımı → sürüm belirleme ve Keep a Changelog çıktısı. Kullanıcıdan baştan daha iyi yapılandırılmış girdi alır. | `642b8be` |
| [conventional-changelog/commitlint](https://github.com/conventional-changelog/commitlint) | 18.755 | TypeScript · MIT | 2026-09-25 | Commit mesajı → kural/preset doğrulaması ve anlaşılır hata. İçeriği son kullanıcı diline çevirmez; girdi kalitesini yükseltir. | `ed3e9ae` |
| [cocogitto/cocogitto](https://github.com/cocogitto/cocogitto) | 1.197 | Rust · MIT | 2026-04-22 | Git geçmişi → Conventional Commit kontrolü, changelog, SemVer bump ve release profilleri. CLI, libgit2 kullanır. | `8cfddce` |
| [pawamoy/git-changelog](https://github.com/pawamoy/git-changelog) | 185 | Python · ISC | 2026-09-22 | Git log → Angular/Conventional/basic stil parser'ları ve Jinja şablonlarıyla sağlayıcıdan bağımsız changelog. | `de83c48` |
| [googleapis/release-please](https://github.com/googleapis/release-please) | 7.554 | TypeScript · Apache-2.0 | 2026-09-14 | Commit geçmişi → güncellenen Release PR, CHANGELOG ve tag. Tek repo veya manifest ile çok bileşenli yapı; hedef branch verilebilir. README karmaşık branch yönetimini kapsam dışı sayıyor. | `edce3d8` |
| [changesets/changesets](https://github.com/changesets/changesets) | 12.444 | TypeScript · MIT | 2026-09-22 | PR sırasında insanın yazdığı küçük `.changeset/*.md` notları → monorepo paketleri için versiyon ve changelog. Commit metninden otomatik anlam çıkarmayı hedeflemez. | `c73949b` |
| [release-drafter/release-drafter](https://github.com/release-drafter/release-drafter) | 3.944 | TypeScript · ISC | 2026-09-23 | Bir branch'e birleştirilen PR'lar → kategori, etiket, başlık ve şablonla sürekli güncellenen release taslağı. CI/GitHub merkezli. | `849a80b` |
| [miniscruff/changie](https://github.com/miniscruff/changie) | 910 | Go · MIT | 2026-09-19 | Dosya tabanlı, insanın yazdığı değişiklik parçaları → yapılandırılabilir toplu release notes. Commit mesajlarıyla changelog'u ayırır. | `e78b7fc` |
| [twisted/towncrier](https://github.com/twisted/towncrier) | 922 | Python · MIT | 2026-09-08 | Issue/PR başına kısa haber parçaları → kategori ve şablonla yayın notu. Karmaşık geliştirici geçmişini kullanıcıya dönük metinden ayırır. | `b8d90be` |
| [git-chglog/git-chglog](https://github.com/git-chglog/git-chglog) | 2.862 | Go · MIT | 2026-01-18 | Tag aralıkları ve Git geçmişi → YAML ile parser/gruplama ve template tabanlı CHANGELOG. **Arşivlenmiş**; README git-cliff'i öneriyor. | `83fc038` |

Raporun bir sonraki yenilemesinde yıldız ve hash değerleri yeniden çekilmelidir.

## Ürün açısından bulgular

### 1. Kaynağa göre üç iş akışı var

- **Git commit'inden üretim:** git-cliff, conventional-changelog, cocogitto, git-changelog ve semantic-release; commit tip/scope/body bilgisini parse eder, filtreler ve sürüm başlıklarında gruplar. Bu yöntem Git geçmişine ve çoğu durumda yerel klona/CI checkout'una dayanır.
- **PR'dan üretim:** Release Drafter PR başlığı, metin ve etiketlerinden taslak oluşturur. Release Please commit mesajlarını analiz edip yayınlanabilir Release PR açar. GitHub API ve CI entegrasyonları işin parçasıdır.
- **İnsan tarafından yazılan change fragment:** Changesets, Changie ve Towncrier, geliştiriciden değişikliğin kullanıcı açısından anlamını kısa bir dosyada kaydetmesini ister. Sonuç genellikle daha temizdir; karşılığında ek katkıcı adımı gerektirir.

Commit Günlüğü'nün mevcut AI akışı ilk iki gruba göre daha serbest: webhook'taki PR/commit metinlerini düzenleme kuyruğuna alıp kullanıcıya dönük sürüm notu taslağı üretiyor. Bu araştırmadaki deterministik araçlar sınıflandırma, sürüm hesabı, şablon ve seçme/eleme kurallarında; insan-notu araçları da editoryal doğrulukta örnek oluşturuyor.

### 2. Yeniden kullanılabilir ürün fikirleri

- Her repo için kaynak branch seçimi ve her branch'in ayrı changelog akışına yönlenmesi; Release Drafter'ın branch başına tetiklenmesi ve Release Please'ın `target-branch`/release-branch seçenekleri örnek alınabilir.
- Conventional Commit tipi, scope, PR etiketi ve dosya yolu ile include/exclude kuralları; teknik bakım commit'lerini kullanıcı notlarından ayırmaya yardım eder.
- Özetleyiciye ham metin yerine ayrıştırılmış olay bağlamı vermek: başlık/body, tür/scope, PR başlığı, issue referansları ve birleştirme bilgisi. Kaynak linkleri kullanıcıya gösterilebilir, ancak üretilen metinde SHA/yol gibi iç ayrıntılar ayıklanmalı.
- Tek bir global çıktı şablonu yerine dil ve repo başına kategori/şablon tercihleri; git-cliff ve git-changelog'un esnekliği burada örnek.
- İnsan tarafından düzenlenen change fragment fikri isteğe bağlı bir editoryal yol olarak değerlendirilebilir; webhook/AI akışının yerine geçmek zorunda değil.

### 3. Local `.git` kaynağı ve branch kapsamı için karar notu

- **Kullanıcı isteği — bu raporda yanıt/feasibility kararı verme:** local `.git` geçmişinden commit toplama fikri daha sonra ele alınacak; hesap ve hosting koşullarıyla ilgili değerlendirme özellikle ertelendi. Bu belge bu başlıkta karar vermez.
- **Branch tercihi ürün gereksinimi olarak kaydedildi:** repo başına branch seçimi gerekli. Mevcut uygulama proje eşlemesini `github_repo_full_name` ile yapıyor; schema'da izlenen branch tercihi yok. Webhook push olaylarında `ref` alanı zaten gelir ve silinmiş/force-push branch temizliği yapılır; bu, event işleme filtresi/branch bazlı stream'in mevcut olduğu anlamına gelmez.
- İlk kapsam kararı gerektiğinde: repo başına tek branch mi, birden fazla branch mi; her branch ayrı public changelog/slug mu; ana branch dışı yayınlar taslakta mı kalır; PR ve release olayları hangi branch'e bağlanır soruları cevaplanmalı. Implementasyon öncesi migration, webhook filtresi, backfill/sync davranışı ve UI birlikte tasarlanmalı.

## Güncel ürünle ilişki

Mevcut proje Rust/Axum + SQLite'tır; modelde `github_repo_full_name` tekil proje anahtarıdır ve webhook entegrasyonu GitHub push/PR/release olaylarını kullanır. Kodu yerel `git` deposuna bağlayan çalışma ağacı veya repo başına branch ayarı bu incelemede bulunmadı. Bu rapor ürün değişikliği değildir; araştırma notudur.

> Önceki rapor taslağında geçen CVE numaraları ve bazı kesin güvenlik iddiaları birincil advisory kaynağıyla doğrulanmadığı için bu sürüme taşınmadı. Bu belge CVE değerlendirmesi değildir.

## VDS klon envanteri

Araştırma kopyaları uygulama çalışma ağacının dışındadır; canlı servise, veritabanına veya API anahtarlarına erişmez. Lisans bilgileri GitHub API metadata'sı/klonlanan LICENSE dosyalarından okunmuştur.

```text
/opt/commit-gunlugu-research/repos/
├── changesets/
├── changie/
├── cocogitto/
├── commitizen/
├── commitlint/
├── conventional-changelog/
├── git-changelog/
├── git-chglog/       # archived upstream
├── git-cliff/
├── release-drafter/
├── release-please/
├── semantic-release/
└── towncrier/
```

Clone'lar `--depth 1` ile alındı. Her repo için lisans, klon başına `LICENSE*` dosyasında; upstream bağlantısı `git remote -v` ile kontrol edilebilir. Güvenlik advisory geçmişi bu araştırmanın kapsamı dışındadır.

## Doğrudan kaynaklar

- [git-cliff README](https://github.com/orhun/git-cliff/blob/main/README.md), [CLI argümanları](https://github.com/orhun/git-cliff/blob/main/git-cliff/src/args.rs)
- [Conventional Changelog README](https://github.com/conventional-changelog/conventional-changelog/blob/master/README.md)
- [semantic-release README](https://github.com/semantic-release/semantic-release/blob/master/README.md)
- [Commitizen README](https://github.com/commitizen-tools/commitizen/blob/master/README.md), [Commitlint README](https://github.com/conventional-changelog/commitlint/blob/master/README.md)
- [Cocogitto README](https://github.com/cocogitto/cocogitto/blob/main/README.md)
- [git-changelog README](https://github.com/pawamoy/git-changelog/blob/main/README.md)
- [Release Please README](https://github.com/googleapis/release-please/blob/main/README.md), [customizing/branch seçenekleri](https://github.com/googleapis/release-please/blob/main/docs/customizing.md)
- [Changesets README](https://github.com/changesets/changesets/blob/main/README.md)
- [Release Drafter README](https://github.com/release-drafter/release-drafter/blob/master/README.md)
- [Changie README](https://github.com/miniscruff/changie/blob/main/README.md)
- [Towncrier dokümantasyonu](https://towncrier.readthedocs.io/)
- [git-chglog arşiv durumu ve README](https://github.com/git-chglog/git-chglog)
