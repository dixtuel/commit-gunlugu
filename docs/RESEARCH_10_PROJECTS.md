# Açık Kaynak Commit, Sürüm ve Changelog Araçlarında Branch (Dal) Yönetimi Mimarisi: 15 Proje Kod Analizi

**Rapor Tarihi:** 2026-09-26  
**Kapsam:** `/opt/commit-gunlugu-research/repos/` altındaki 15 popüler açık kaynak projenin kaynak kodları, konfigürasyon şemaları ve CLI motorları.  
**Amaç:** Açık kaynak changelog, commit analiz ve release araçlarının branch ayrımını (tek branch, çoklu branch, glob desenleri, whitelist/blacklist, merge-base izolasyonu) nasıl uyguladığını kod düzeyinde incelemek ve Commit Günlüğü ürünü için en doğru çoklu branch seçim mimarisini belirlemek.

---

## 1. Yönetici Özeti ve Temel Çıkarımlar

İncelenen 15 projenin tamamında Git branch mimarisine yaklaşım 4 ana tasarım desenine ayrılmaktadır:

1. **Çoklu Branch Listesi ve Desen Eşleme (Multi-Branch Whitelist / Glob):**  
   - Öncüler: `semantic-release`, `cocogitto`, `release-it`, `auto`.  
   - Kullanıcı `["main", "next", "beta", "release/*"]` gibi bir liste tanımlar. Araç, olayın ya da geçerli çalışma dizininin branch'ini bu listedeki isimlerle veya glob/regex kalıplarıyla karşılaştırır. Eşleşmeyen olaylar/branch'ler sessizce atlanır veya hata verir.
2. **Hedef Dal (Target Branch / Commitish) İzolasyonu:**  
   - Öncüler: `release-please`, `release-drafter`.  
   - Her changelog akışı belirli bir hedef branch'e (`commitish`) bağlanır. PR'lar `base.ref` üzerinden, push'lar ise `ref` üzerinden filtrelenir (`filter-by-commitish: true`).
3. **Branch Ağacı ve Ata/Miras Denetimi (Reachability & Ancestry Filter):**  
   - Öncüler: `git-cliff`, `git-changelog`, `commitizen`.  
   - Sadece branch adına bakmak yerine, commit ve tag'lerin o dalın `HEAD` commit'inin soyundan gelip gelmediği (`git merge-base`, `should_include_tag`) kontrol edilir (`use_branch_tags = true`). Böylece farklı feature branch'lerindeki tag ve commit'lerin ana dalın changelog'una sızması engellenir.
4. **Dosya Tabanlı Dal İzolasyonu (Fragment-per-Branch):**  
   - Öncüler: `changesets`, `changie`, `towncrier`.  
   - Her branch kendi unreleased değişiklik dosyalarını (`.changeset/`, `.changes/unreleased/`) taşır. Ana dala merge edilene kadar o branch'in notları bağımsız kalır; `--since <branch>` veya `baseBranch` ile kıyaslama yapılır.

---

## 2. 15 Projenin Detaylı Kod Analizi

### 1. `semantic-release` (JavaScript / Node.js)
- **Repo:** [semantic-release/semantic-release](https://github.com/semantic-release/semantic-release)
- **Branch Konfigürasyon Türü:** Dizi / Regex / Obje (`Array<string | RegExp | BranchObject>`)
- **Konfigürasyon Örneği:**
  ```json
  {
    "branches": [
      "main",
      "next",
      { "name": "beta", "prerelease": true },
      { "name": "v+([0-9]+).x", "channel": "${name}" }
    ]
  }
  ```
- **Kod İncelemesi (`lib/branches/expand.js`, `lib/branches/index.js`):**
  `expand.js` içinde `micromatch` kütüphanesi kullanılarak uzak depodaki tüm Git branch'leri çekilir (`gitBranches = await getBranches(...)`) ve kullanıcının `branches` listesindeki glob desenleriyle eşleştirilir. Geçerli CI branch'i bu listede yoksa release adımı işletilmez.

---

### 2. `cocogitto` (Rust)
- **Repo:** [cocogitto/cocogitto](https://github.com/cocogitto/cocogitto)
- **Branch Konfigürasyon Türü:** `Vec<String>` (Glob desenleri listesi)
- **Konfigürasyon Örneği (`cog.toml`):**
  ```toml
  branch_whitelist = ["main", "master", "release/**"]
  ```
- **Kod İncelemesi (`crates/cocogitto/src/command/bump/mod.rs:280`):**
  ```rust
  if !SETTINGS.branch_whitelist.is_empty() {
      if let Some(branch) = self.repository.get_branch_shorthand() {
          let whitelist = &SETTINGS.branch_whitelist;
          let is_match = whitelist.iter().any(|pattern| {
              let glob = Glob::new(pattern).expect("invalid glob pattern").compile_matcher();
              glob.is_match(&branch)
          });
          ensure!(is_match, "Current branch '{}' is not whitelisted", branch);
      }
  }
  ```
  Boş liste (`[]`) tüm branch'lere izin verir. Dolu olduğunda `glob.is_match(&branch)` ile doğrulanır.

---

### 3. `release-it` (JavaScript)
- **Repo:** [release-it/release-it](https://github.com/release-it/release-it)
- **Branch Konfigürasyon Türü:** Tek string veya string dizisi (`git.requireBranch`), negatifleme (`!`) desteği
- **Konfigürasyon Örneği:**
  ```json
  {
    "git": {
      "requireBranch": ["main", "release/*", "!next"]
    }
  }
  ```
- **Kod İncelemesi (`lib/plugin/git/Git.js:98-109`):**
  ```javascript
  async isRequiredBranch() {
    const branch = await this.getBranchName();
    const requiredBranches = castArray(this.options.requireBranch);
    const [branches, negated] = requiredBranches.reduce(
      ([p, n], b) => (b.startsWith('!') ? [p, [...n, b.slice(1)]] : [[...p, b], n]),
      [[], []]
    );
    return (
      (branches.length > 0 ? matcher(branches)(branch) : true) &&
      (negated.length > 0 ? !matcher(negated)(branch) : true)
    );
  }
  ```

---

### 4. `auto` (Intuit / TypeScript)
- **Repo:** [intuit/auto](https://github.com/intuit/auto)
- **Branch Konfigürasyon Türü:** `baseBranch` (string) + `prereleaseBranches` (`string[]`)
- **Konfigürasyon Örneği (`.autorc`):**
  ```json
  {
    "baseBranch": "main",
    "prereleaseBranches": ["next", "beta", "alpha"]
  }
  ```
- **Kod İncelemesi (`packages/core/src/auto.ts`):**
  Varsayılan ana branch (`main` / `master`) kararlı sürüm notları için kullanılır. `prereleaseBranches` dizisindeki branch'lerden tetiklenen değişiklikler izole prerelease changelog'u oluşturur ve ana daldan ayrıştırılır.

---

### 5. `release-please` (Google / TypeScript)
- **Repo:** [googleapis/release-please](https://github.com/googleapis/release-please)
- **Branch Konfigürasyon Türü:** `--target-branch` parametresi ve çoklu manifest `branches` listesi
- **Konfigürasyon Örneği (`.github/release-please.yml`):**
  ```yaml
  branches:
    - branch: main
      releaseType: rust
    - branch: 12.x
      releaseType: node
  ```
- **Kod İncelemesi (`src/bin/release-please.ts:183`, `src/manifest.ts`):**
  Her manifest girdisi kendi `branch` hedefine sahiptir. GitHub API üzerinden PR'lar açılırken ve tag'ler oluşturulurken doğrudan belirtilen hedef branch esas alınır; diğer branch'lerdeki commit'ler o hedefin PR'ına dahil edilmez.

---

### 6. `release-drafter` (TypeScript / GitHub Actions)
- **Repo:** [release-drafter/release-drafter](https://github.com/release-drafter/release-drafter)
- **Branch Konfigürasyon Türü:** `commitish` (string) + `filter-by-commitish: boolean`
- **Konfigürasyon Örneği (`.github/release-drafter.yml`):**
  ```yaml
  commitish: main
  filter-by-commitish: true
  ```
- **Kod İncelemesi (`packages/core/src/config/config.schema.ts`):**
  GitHub webhook'u veya Action çalıştığında, PR'ın birleştirildiği hedef dal (`pull_request.base.ref`) `commitish` ile kıyaslanır. `filter-by-commitish: true` ise, önceki yayınlar aranırken de yalnızca bu branch'e hedeflenmiş release'ler dikkate alınır.

---

### 7. `git-cliff` (Rust)
- **Repo:** [orhun/git-cliff](https://github.com/orhun/git-cliff)
- **Branch Konfigürasyon Türü:** CLI revision range (`git cliff main..HEAD`) + `use_branch_tags: bool`
- **Konfigürasyon Örneği (`cliff.toml`):**
  ```toml
  [git]
  use_branch_tags = true
  topo_order = true
  ```
- **Kod İncelemesi (`git-cliff-core/src/repo.rs:565-595`):**
  `use_branch_tags` devredeyken libgit2 `graph_descendant_of` fonksiyonu çağrılır. İncelenen tag commit'i, mevcut branch'in tepe commit'inin (`head_commit`) atası değilse elenir. Böylece yan dallarda (feature branch) atılan tag'ler ana changelog'u bozmaz.

---

### 8. `changesets` (TypeScript)
- **Repo:** [changesets/changesets](https://github.com/changesets/changesets)
- **Branch Konfigürasyon Türü:** `baseBranch` (string) + CLI `--since <branch>`
- **Konfigürasyon Örneği (`.changeset/config.json`):**
  ```json
  {
    "baseBranch": "main"
  }
  ```
- **Kod İncelemesi (`packages/cli/src/commands/add/index.ts`):**
  `--since <branch>` argümanı Gitflow iş akışlarında çoklu hedef dal kullanımını destekler. Paketlerdeki değişiklikler `baseBranch` ile mevcut branch arasındaki Git diff üzerinden tespit edilir.

---

### 9. `commitizen` (Python)
- **Repo:** [commitizen-tools/commitizen](https://github.com/commitizen-tools/commitizen)
- **Branch Konfigürasyon Türü:** `--rev-range <branch>..HEAD` ve `get_default_branch()`
- **Kod İncelemesi (`commitizen/git.py:354`, `commitizen/commands/check.py`):**
  CLI üzerinden belirtilen branch aralığındaki commit'ler denetlenir. Deponun varsayılan branch'i (`origin/HEAD` veya `main/master`) otomatik çözümlenir.

---

### 10. `git-changelog` (Python)
- **Repo:** [pawamoy/git-changelog](https://github.com/pawamoy/git-changelog)
- **Branch Konfigürasyon Türü:** Çoklu release/develop dal tespiti
- **Kod İncelemesi (`src/git_changelog/_internal/providers.py`):**
  Sağlayıcı (GitHub/GitLab) API'si üzerinden branch karşılaştırma linkleri üretilir (`/branches/compare/{ref}`). Birleştirilen dallardaki mükerrer commit'ler tekilleştirilir.

---

### 11. `git-chglog` (Go - Arşivlenmiş)
- **Repo:** [git-chglog/git-chglog](https://github.com/git-chglog/git-chglog)
- **Branch Konfigürasyon Türü:** Merge commit regex ayrıştırma
- **Kod İncelemesi (`chglog.go:82`, `cmd/git-chglog/config.go:220`):**
  ```go
  opts.Merges.Pattern = "^Merge branch '.*' into '(.*)'$"
  ```
  Merge commit mesajından hedef dal adı okunur ve commit'ler hedeflenen branch'e göre gruplanır.

---

### 12. `towncrier` (Python)
- **Repo:** [twisted/towncrier](https://github.com/twisted/towncrier)
- **Branch Konfigürasyon Türü:** `get_default_compare_branch`
- **Kod İncelemesi (`src/towncrier/_git.py:13`):**
  `origin/main` veya `origin/master` dallarını arar; yerel değişiklik parçalarını (news fragments) karşılaştırma dalına göre süzerek changelog derlemesi yapar.

---

### 13. `conventional-changelog` (TypeScript)
- **Repo:** [conventional-changelog/conventional-changelog](https://github.com/conventional-changelog/conventional-changelog)
- **Branch Konfigürasyon Türü:** `@conventional-changelog/git-client` rev yürüteci
- **Kod İncelemesi (`packages/git-client/src/types.ts`):**
  `from` ve `to` ref'leri üzerinden branch geçmişi taranır. `--abbrev=0` ile yalnızca aynı daldaki tag'lerin alınması sağlanır.

---

### 14. `changie` (Go)
- **Repo:** [miniscruff/changie](https://github.com/miniscruff/changie)
- **Branch Konfigürasyon Türü:** Dala özel unreleased dosyaları
- **Kod İncelemesi:**
  Değişiklikler `.changes/unreleased/` altında bağımsız dosyalardır. Branch bazında açılan PR'larla ana dala akar, batch komutu o anki aktif daldaki dosyaları birleştirir.

---

### 15. `commitlint` (TypeScript)
- **Repo:** [conventional-changelog/commitlint](https://github.com/conventional-changelog/commitlint)
- **Branch Konfigürasyon Türü:** `--from <branch> --to <branch>`
- **Kod İncelemesi (`@commitlint/read`):**
  İki dal arasındaki ortak ata (`merge-base`) bulunamazsa işlem başarısız olur; dallar arası commit deltası doğrulanır.

---

## 3. Karşılaştırma Matrisi

| # | Proje | Çoklu Branch Desteği | Tanımlama Biçimi | Eşleme Yöntemi | Boş / Varsayılan Davranış |
|---|---|:---:|---|---|---|
| 1 | **semantic-release** | Evet | Dizi / Regex / Obje | `micromatch` glob | Varsayılan: `['master', 'main']` |
| 2 | **cocogitto** | Evet | Glob dizisi (`branch_whitelist`) | Rust `glob::compile_matcher` | Boş = Tüm branch'ler serbest |
| 3 | **release-it** | Evet | String / Dizi (`requireBranch`) | `matcher` + `!` negation | Boş = Herhangi bir branch |
| 4 | **auto** | Evet | `baseBranch` + Dizi (`prereleaseBranches`) | String equality | Varsayılan: `main` |
| 5 | **release-please** | Evet | Manifest dizi (`branches`) | Exact match per package | Deponun varsayılan dalı |
| 6 | **release-drafter** | Evet (Çoklu Action) | String (`commitish`) | PR `base.ref` equality | Workflow dalı (`main`) |
| 7 | **git-cliff** | Evet | CLI Range + `use_branch_tags` | `graph_descendant_of` soy ağacı | Aktif branch / tüm tag'ler |
| 8 | **changesets** | Evet | `baseBranch` + `--since <branch>` | Git diff range | `main` / `master` |
| 9 | **commitizen** | Evet | CLI `--rev-range <branch>..HEAD` | Git rev-list | `get_default_branch()` |
| 10 | **git-changelog** | Evet | Çoklu release/develop branch | Regex / URL builder | `master` / `main` |
| 11 | **git-chglog** | Evet | Merge commit regex pattern | Regex capture group | Tüm merge commit'leri |
| 12 | **towncrier** | Dolaylı | Karşılaştırma dalı tespiti | Remote ref comparison | `origin/main` |
| 13 | **conventional-changelog** | Evet | Git client ref walk | Rev list parsing | HEAD / Aktif branch |
| 14 | **changie** | Dolaylı | Dosya tabanlı dal merge'i | Dizin okuma | Aktif dal unreleased |
| 15 | **commitlint** | Evet | `--from <branch> --to <branch>` | Merge-base rev walk | HEAD~1 .. HEAD |

---

## 4. Commit Günlüğü İçin Mimari Karar ve Tasarım

15 projenin mimarilerinden çıkarılan en temiz ve dayanıklı sentez şudur:

1. **Depolama ve Model:**
   - Proje düzeyinde: Kullanıcı ister tek branch (`main`), ister 2-3 veya daha fazla branch (`main`, `dev`, `release/1.x`) seçebilir.
   - Girdi hem virgülle (`main, dev, staging`) hem alt alta satırlarla (`\n`) yazılabilir.
   - Boş bırakılması durumunda (`""`) **tüm branch'ler** izlenmeye devam eder (geriye dönük tam uyumluluk).
2. **Çoktan-Çoğa (Many-to-Many) İlişki Tablosu:**
   - Bir commit veya sürüm notu birden fazla branch'e ait olabilir (örn. `cherry-pick` veya `merge` durumları).
   - `entry_branches (entry_id, project_id, branch_name)` tablosu ile her girdinin hangi branch'lere ait olduğu takip edilir.
3. **Webhook Filtreleme:**
   - Push: `payload.ref` (`refs/heads/<branch>`) kontrol edilir.
   - PR: `pull_request.base.ref` (hedef branch) kontrol edilir.
   - Release: `release.target_commitish` kontrol edilir.
   - Projenin seçili branch listesi doluysa ve gelen branch bu listede yoksa, olay sessizce atlanır (diğer branch'lerin gürültüsü engellenir).
4. **Kullanıcı Arayüzü (UI):**
   - Hem proje ekleme hem proje düzenleme modalında hem elle metin alanı (virgül/satır destekli) hem de GitHub API'den projenin gerçek branch'lerini çekip çoklu seçmeye (multi-select) imkan tanıyan buton/seçici bulunur.
   - Dashboard kartında projenin takip ettiği branch'ler açıkça gösterilir.
5. **Herkese Açık Sayfa (`/c/:slug`) & Filtreleme:**
   - Ziyaretçiler projenin birden fazla takip edilen branch'i varsa URL'den `?branch=...` parametresiyle veya arayüzden spesifik bir dala ait sürüm notlarını filtreleyebilir.
