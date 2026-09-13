# Açık Kaynak Lisansları ve Atıflar (ATTRIBUTION)

Bu belge, **Commit Günlüğü** projesinde (`commit-gunlugu`) kullanılan tüm açık kaynaklı kütüphaneleri, algoritmaları, mimari referansları ve ilgili lisans şartlarını belgeler.

---

## 1. Mimari İlhamlar ve Tasarım Referansları

- **Conventional Commits 1.0.0:** Sürüm günlüğü kategorilendirme ve deterministik kural motoru Conventional Commits (`feat:`, `fix:`, `perf:`, `refactor:`) spesifikasyonuna dayanır.
- **Mikoshi AI (VDS Altyapısı):** KVKK uyumlu hesap silme, imha ledger'ı (`erasure_ledger`) ve opsiyonel bulut yedeğinden kurtarma sonrasında silinmiş hesapların dirilmesini önleyen restore-hook mimarisi, ayrıca yerel SMTP aktarıcısı üzerinden şifre sıfırlama e-postası gönderim deseni Mikoshi AI'ın altyapı standartlarından uyarlanmıştır (bu proje kendi altyapı kimlik bilgilerini içermez, uyarlanan yalnızca mimari desendir).
- **randomservice (Rust / Axum 0.7):** Sub-millisecond asenkron webhook karşılama, Leaky-Bucket IP hız kısıtlaması (`tower_governor`) ve 3 kademeli (NVIDIA NIM &rarr; Gateway &rarr; Deterministik) yapay zeka fallback zinciri `randomservice` mimarisinden esinlenilmiştir.
- **GitHub Webhook Security Best Practices:** Sabit zamanlı HMAC-SHA256 imza doğrulaması ve `X-GitHub-Delivery` benzersizlik kontrolü GitHub resmi dokümantasyon standartlarına uygundur.

---

## 2. Kullanılan Rust Kütüphaneleri (Crates) ve Lisanslar

| Kütüphane | Sürüm | Lisans | Sorumluluk |
| :--- | :--- | :--- | :--- |
| `axum` | 0.7 | MIT | Web uygulama çerçevesi ve HTTP yönlendirme |
| `tokio` | 1.x | MIT | Çok iş parçacıklı asenkron çalışma zamanı ve görev kuyruğu |
| `sqlx` | 0.8 | Apache-2.0 / MIT | SQLite (WAL mod) asenkron veritabanı sürücüsü ve migrasyonlar |
| `tower` & `tower-http` | 0.5 / 0.6 | MIT | CORS, trace, statik dosya sunumu ve ara katmanlar |
| `tower_governor` | 0.4 | MIT | Akıllı IP anahtarlı Leaky-Bucket DoS ve brute-force hız sınırlayıcı |
| `minijinja` | 2.x | Apache-2.0 | Hızlı, sıfır bağımlılıklı sunucu taraflı HTML şablon motoru |
| `argon2` & `password-hash` | 0.6 | Apache-2.0 / MIT | OWASP standartlarında parola hashleme ve doğrulama |
| `sha2` & `hmac` | 0.10 / 0.12 | Apache-2.0 / MIT | Kriptografik SHA-256 ve GitHub Webhook HMAC imza doğrulaması |
| `subtle` | 2.6 | BSD-3-Clause | Zamanlama saldırılarını (timing attack) önleyen sabit zamanlı karşılaştırma |
| `reqwest` | 0.12 | Apache-2.0 / MIT | Asenkron HTTPS istemcisi (NVIDIA NIM ve AI Gateway bağlantısı) |
| `serde` & `serde_json` | 1.0 | Apache-2.0 / MIT | JSON serileştirme ve veri ayrıştırma |
| `dotenvy` | 0.15 | MIT | Çevre değişkenleri (.env) yükleyici |
| `tracing` & `tracing-subscriber` | 0.3 | MIT | Yapılandırılmış loglama ve gözlemlenebilirlik |
| `uuid` | 1.x | Apache-2.0 / MIT | Benzersiz kimlik üretimi (v4) |
| `chrono` | 0.4 | Apache-2.0 / MIT | Zaman ve tarih işlemleri |
| `regex` | 1.x | Apache-2.0 / MIT | E-posta ve hassas veri filtreleme ifadeleri |
| `aes-gcm` | 0.10 | Apache-2.0 / MIT | Kullanıcıların özel GitHub PAT'lerinin at-rest AES-256-GCM şifrelemesi |
| `lettre` | 0.11 | MIT | Opsiyonel SMTP üzerinden şifre sıfırlama e-postası gönderimi |

---

## 3. Yazı Tipleri (Google Fonts, SIL Open Font License 1.1)

Arayüz `https://fonts.googleapis.com` üzerinden aşağıdaki açık kaynak yazı tiplerini yükler:

- **Fraunces** — editoryal başlıklar ve yayınlanmış sürüm notu metinleri
- **Work Sans** — arayüz/gövde metni
- **IBM Plex Mono** — ham veri, commit SHA'ları ve rozetler

Üçü de SIL Open Font License 1.1 altında lisanslıdır.

---

## 4. Lisans Metinleri

### MIT License

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache License 2.0

```text
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
