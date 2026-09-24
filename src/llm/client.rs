use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::llm::deterministic::{generate_deterministic_entry, EntryDraft};
use crate::sanitizer::sanitize_text;

pub const SYSTEM_PROMPT_TR: &str = r#"Sen kıdemli bir teknik ürün editörüsün ("Seyir Defteri" editörü). Görevin; ham Git commit mesajlarını ve Pull Request verilerini, yazılım ürününü kullanan son kullanıcılar için anlaşılır, editoryal ve değer odaklı bir sürüm günlüğü (changelog) kaydına dönüştürmektir.

## Temel Kurallar ve Prensipler:
1. DEĞER ODAKLI ÇEVİRİ:
   - Geliştirici jargonunu (refactor, dependency bump, regex, query optimization, null check vb.) son kullanıcının doğrudan hissedeceği faydaya dönüştür.
   - "Ne yapıldı?" yerine "Kullanıcı için ne iyileşti / ne kolaylaştı?" sorusuna odaklan.

2. EDİTORYAL TON ("SEYİR DEFTERİ" FELSEFESİ):
   - Sade, profesyonel, sakin ve net bir Türkçe kullan.
   - KESİNLİKLE EMOJİ KULLANMA (🚀, 🐛, ⚡, ✨, 🎉, 🔧 vb. tamamen yasaktır).
   - Abartılı pazarlama sıfatlarından ("devrim niteliğinde", "mükemmel deneyim") ve uydurma kurumsal özelliklerden kaçın; sadece yapılan işin gerçek etkisini anlat.

3. GİZLİLİK VE VERİ TEMİZLİĞİ:
   - Commit hash'leri (SHA), dahili dosya yolları (src/...), branch adları, geliştirici adları veya e-posta adreslerini ASLA metne dahil etme.

4. KATEGORİ TESPİTİ VE NİYET ANALİZİ (Yalnızca şu 3 değerden biri):
   - Geliştiriciler "feat", "fix", "chore" gibi Conventional Commit önekleri KULLANMAMIŞ OLABİLİR. Günlük konuşma diliyle veya öneksiz serbest yazılmış mesajları derinlemesine analiz et ve yapılan işin özündeki niyeti belirle:
     * "NEW": Kullanıcının önceden deneyimleyemediği yeni bir ekran, sayfa, buton, yetenek, entegrasyon veya fonksiyon sisteme eklenmişse. (Örnekler: "canlı destek geldi", "Google OAuth entegrasyonu", "PDF çıktısı alma", "karanlık tema eklendi", "arama çubuğu", "yeni üyelik akışı", "bildirim zili").
     * "FIX": Önceden bozuk, çöken, kilitlenen, görsel olarak kayan/taşan, çalışmayan, hatalı hesaplayan veya aksayan bir problem onarılmışsa. (Örnekler: "sepet donma problemi çözüldü", "mobilde buton taşması engellendi", "şifre sıfırlama linki gitmiyordu", "NullPointer hatası", "arama boş geliyordu", "crash sorunu", "bellek sızıntısı", "fatura tutarı yanlış hesaplanıyordu").
     * "IMPROVEMENT": Var olan bir yapının daha hızlı, daha akıcı, daha temiz veya daha güvenli hale getirilmesi, altyapı/kütüphane güncellemesi, tasarım cilası veya optimizasyon yapılmışsa. (Örnekler: "sayfa açılışı hızlandırıldı", "ikonlar yenilendi", "veritabanı sorguları optimize edildi", "mobil arayüz sadeleştirildi", "refactor", "kod temizliği").
   - Çoklu veya karma commit'lerde (ör. hem hata düzeltmesi hem yeni bir yetenek varsa), son kullanıcı için en belirgin ve değerli olan etkiyi seç.

## Çıktı Formatı (JSON Only):
Yanıtın YALNIZCA geçerli ve parse edilebilir bir JSON nesnesi olmalıdır. Yanıtına markdown kod bloğu (```json), selamlama veya düşünce (reasoning) açıklaması EKLEME.

Örnek 1 (Öneksiz / Serbest Hata Düzeltme Girişi):
Commit Mesajları:
- sepette kupon kodu girince sayfa kilitleniyordu çözüldü
- indirim hesaplama fonksiyonundaki sıfıra bölünme aksaklığı giderildi
JSON Çıktısı:
{
  "category": "FIX",
  "title": "Kupon Kodu ve Sepet Hatası Giderildi",
  "body": "Sepet adımında indirim kuponu girildiğinde oluşan sayfa kilitlenmesi ve hesaplama hatası düzeltildi."
}

Örnek 2 (Öneksiz / Serbest Yeni Özellik Girişi):
Commit Mesajları:
- profile iki adımlı doğrulama (2FA) sekmesi geldi
- authenticator qr kod üretimi ve yedek kodlar
JSON Çıktısı:
{
  "category": "NEW",
  "title": "İki Adımlı Doğrulama (2FA) Desteği",
  "body": "Hesap güvenliğini artırmak için Authenticator uygulamalarıyla uyumlu iki adımlı doğrulama ve yedek kod oluşturma özelliği kullanıma sunuldu."
}

Örnek 3 (Öneksiz / Serbest İyileştirme Girişi):
Commit Mesajları:
- veritabanı bağlantı havuzu yenilendi ve sorgular önbelleğe alındı
- sayfa geçişleri belirgin şekilde hızlandı
JSON Çıktısı:
{
  "category": "IMPROVEMENT",
  "title": "Sayfa Açılış Hızı ve Altyapı Kararlılığı",
  "body": "Veritabanı sorguları ve önbellekleme mekanizması optimize edilerek sayfa açılış ve geçiş süreleri hızlandırıldı."
}"#;

pub const SYSTEM_PROMPT_EN: &str = r#"You are a senior technical product editor ("Ship Log" editor). Your mission is to transform raw Git commit messages and Pull Request data into a clear, editorial, and value-driven changelog entry for end users of the software product.

## Core Rules and Principles:
1. VALUE-DRIVEN TRANSLATION:
   - Translate developer jargon (refactor, dependency bump, regex, query optimization, null check, etc.) into direct end-user benefits.
   - Focus on "What improved or became easier for the user?" instead of "What was done under the hood?".

2. EDITORIAL TONE ("LOGBOOK" PHILOSOPHY):
   - Use clean, professional, calm, and concise English.
   - ABSOLUTELY NO EMOJIS (no 🚀, 🐛, ⚡, ✨, 🎉, 🔧, etc.).
   - Avoid exaggerated marketing fluff ("revolutionary", "game-changing") and corporate buzzwords; convey only the genuine impact of the changes.

3. PRIVACY AND HYGIENE:
   - NEVER include commit hashes (SHAs), internal file paths (src/...), branch names, developer names, or email addresses in the output text.

4. CATEGORY IDENTIFICATION AND INTENT ANALYSIS (Only one of these 3 values):
   - Developers MAY NOT have used Conventional Commits prefixes (feat, fix, chore). Deeply analyze free-form or conversational commit messages to identify the semantic intent:
     * "NEW": A brand new screen, page, button, capability, integration, or feature that users couldn't experience before. (Examples: "added google oauth", "two-factor authentication", "export to pdf", "dark theme support", "search bar", "notification bell").
     * "FIX": A problem that was previously broken, crashing, freezing, visually overflowing, malfunctioning, or miscalculating has been repaired. (Examples: "checkout freezing issue resolved", "fixed button overflow on mobile", "password reset email was not sending", "NullPointerException", "empty search results", "memory leak", "invoice amount miscalculation").
     * "IMPROVEMENT": Making an existing system faster, cleaner, more secure, or more reliable, upgrading dependencies, design polish, or optimization. (Examples: "faster page load times", "refreshed icons", "optimized database queries", "streamlined mobile interface", "refactoring", "code cleanup").
   - For multiple or mixed commits, choose the category that delivers the most significant and tangible value to the end user.

## Output Format (JSON Only):
Your response MUST be ONLY a valid and parseable JSON object. DO NOT wrap in markdown codeblocks (```json), add greetings, or include chain-of-thought/reasoning blocks.

Example 1 (Free-form Bug Fix):
Commit Messages:
- coupon code input was freezing checkout page
- fixed divide by zero error in discount calculation
JSON Output:
{
  "category": "FIX",
  "title": "Checkout Coupon Issue Resolved",
  "body": "Fixed an issue where entering a discount coupon during checkout caused the page to freeze or calculate totals incorrectly."
}

Example 2 (Free-form New Feature):
Commit Messages:
- two-factor authentication (2FA) tab added to profile
- authenticator qr code generation and backup codes
JSON Output:
{
  "category": "NEW",
  "title": "Two-Factor Authentication (2FA) Support",
  "body": "Enhanced account security with authenticator-compatible two-factor authentication and backup recovery codes."
}

Example 3 (Free-form Improvement):
Commit Messages:
- refreshed database connection pool and cached queries
- page transitions are significantly faster
JSON Output:
{
  "category": "IMPROVEMENT",
  "title": "Faster Page Load and System Stability",
  "body": "Optimized database queries and caching mechanisms to significantly improve page loading and navigation speeds."
}"#;

/// Gelen metin içerisindeki Türkçe karakteristik karakter ve kelimeleri analiz ederek
/// dilin Türkçe ("tr") mi yoksa İngilizce ("en") mi olduğunu yüksek doğrulukla tespit eder.
pub fn detect_text_language(text: &str) -> &'static str {
    let lower = text.to_lowercase();

    // 1. Türkçe'ye özgü harfler (ç, ğ, ı, ö, ş, ü) - çok güçlü sinyal
    let turkish_chars = ['ç', 'ğ', 'ı', 'ö', 'ş', 'ü'];
    let mut tr_char_count = 0;
    for c in lower.chars() {
        if turkish_chars.contains(&c) {
            tr_char_count += 1;
        }
    }
    if tr_char_count >= 2 {
        return "tr";
    }

    // 2. Türkçe yaygın fiil çekimleri, bağlaçlar ve kelimeler
    let tr_words = [
        "ve", "ile", "için", "düzeltildi", "eklendi", "güncellendi", "hata", "çözüldü",
        "yapıldı", "sağlandı", "yeni", "artık", "sayfa", "kullanıcı", "buton", "ayar",
        "sepet", "giriş", "düzelt", "sorun", "iyileştir", "kodu", "paneli", "destek",
        "geliştirme", "kaldırıldı", "değişiklik", "ekle", "geldi", "düzeltme", "onarıldı"
    ];

    let en_words = [
        "the", "and", "for", "with", "from", "fix", "fixed", "add", "added", "update",
        "updated", "feat", "chore", "remove", "removed", "refactor", "support", "feature",
        "release", "bump", "improve", "improved", "resolve", "resolved", "issue", "crash",
        "bug", "button", "screen", "page", "user", "authentication", "login", "merge"
    ];

    let mut tr_score = tr_char_count * 2;
    let mut en_score = 0;

    for w in &tr_words {
        if lower.contains(w) {
            tr_score += 1;
        }
    }

    for w in &en_words {
        if lower.contains(w) {
            en_score += 1;
        }
    }

    if tr_score > en_score && tr_score > 0 {
        "tr"
    } else {
        "en"
    }
}

/// Projenin kayıtlı dil ayarı ("tr", "en", "auto") ile gelen commit metinlerini
/// harmanlayarak modelin çalışacağı nihai hedef dili ("tr" veya "en") çözer.
pub fn resolve_target_language(project_lang: Option<&str>, sample_text: &str) -> &'static str {
    match project_lang.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("tr") => "tr",
        Some("en") => "en",
        _ => detect_text_language(sample_text),
    }
}

/// Hedef dile uygun ("tr" veya "en") tam editoryal system prompt'unu döner.
pub fn get_system_prompt(lang: &str) -> &'static str {
    if lang == "en" {
        SYSTEM_PROMPT_EN
    } else {
        SYSTEM_PROMPT_TR
    }
}

#[derive(Clone)]
struct ModelProfile {
    name: String,
    temperature: f32,
    top_p: Option<f32>,
    max_tokens: u32,
    timeout_ms: u64,
    extra: Value,
}

const NIM_CHAIN_TIMEOUT_MS: u64 = 45_000;

fn known_profile(name: &str) -> ModelProfile {
    match name {
        "nvidia/nemotron-3.5-lightning-30b-a3b" => ModelProfile {
            name: name.to_string(),
            temperature: 0.6,
            top_p: Some(0.95),
            max_tokens: 1024,
            timeout_ms: 30_000,
            // reasoning_budget=0: Sonsuz düşünce (reasoning) döngüsünü kapatıp anında editoryal JSON üretmesini sağlar.
            extra: json!({ "reasoning_budget": 0 }),
        },
        "meta/muse-glimmer-30b" => ModelProfile {
            name: name.to_string(),
            temperature: 0.95,
            top_p: Some(1.0),
            max_tokens: 1024,
            timeout_ms: 30_000,
            extra: json!({
                "reasoning_effort": "low",
                "chat_template_kwargs": { "reasoning_strength": "low" }
            }),
        },
        "openai/gpt-oss-20b" => ModelProfile {
            name: name.to_string(),
            temperature: 0.7,
            top_p: Some(1.0),
            max_tokens: 1024,
            timeout_ms: 30_000,
            extra: json!({}),
        },
        "poolside/laguna-xs-2.1" => ModelProfile {
            name: name.to_string(),
            temperature: 0.8,
            top_p: Some(0.95),
            max_tokens: 1024,
            timeout_ms: 30_000,
            extra: json!({}),
        },
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning" => ModelProfile {
            name: name.to_string(),
            temperature: 0.6,
            top_p: Some(0.95),
            max_tokens: 65536,
            timeout_ms: 30_000,
            extra: json!({ "reasoning_budget": 16384 }),
        },
        "google/gemma-4-31b-it" => ModelProfile {
            name: name.to_string(),
            temperature: 0.5,
            top_p: Some(1.0),
            max_tokens: 2048,
            timeout_ms: 30_000,
            extra: json!({
                "chat_template_kwargs": { "enable_thinking": false }
            }),
        },
        "z-ai/glm-5.3" => ModelProfile {
            name: name.to_string(),
            temperature: 0.5,
            top_p: None,
            max_tokens: 2048,
            timeout_ms: 12_000,
            extra: json!({
                "reasoning_effort": "low",
                "chat_template_kwargs": { "clear_thinking": true }
            }),
        },
        "nvidia/nemotron-3-super-120b-a12b" => ModelProfile {
            name: name.to_string(),
            temperature: 1.0,
            top_p: Some(0.95),
            max_tokens: 3072,
            timeout_ms: 12_000,
            extra: json!({ "reasoning_effort": "low" }),
        },
        other => ModelProfile {
            name: other.to_string(),
            temperature: 0.7,
            top_p: Some(1.0),
            max_tokens: 1024,
            timeout_ms: 30_000,
            extra: json!({}),
        },
    }
}

#[derive(Clone)]
pub struct LlmFallbackEngine {
    http: reqwest::Client,
    config: Config,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    reasoning_content: Option<String>,
}

impl LlmFallbackEngine {
    pub fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { http, config }
    }

    /// Çok aşamalı AI zinciri:
    /// 1. Dil tespiti ve hedef dile uygun system prompt seçimi (TR / EN)
    /// 2. NVIDIA NIM modelleri (sırayla denenir)
    /// 3. Deterministik kural motoru (Zero-failure)
    pub async fn summarize(
        &self,
        project_lang: Option<&str>,
        pr_title: Option<&str>,
        pr_body: Option<&str>,
        commit_messages: &[String],
    ) -> EntryDraft {
        // Girdi temizliği: e-postaları ve hassas kalıntıları filtrele
        let clean_commits: Vec<String> = commit_messages
            .iter()
            .map(|m| sanitize_text(m))
            .collect();
        let clean_pr_title = pr_title.map(sanitize_text);
        let clean_pr_body = pr_body.map(sanitize_text);

        // Dil çözümlemesi (Proje ayarı "tr" | "en" ise doğrudan o dil; "auto" ise metin analizi)
        let mut sample_for_lang = String::new();
        if let Some(ref t) = clean_pr_title {
            sample_for_lang.push_str(t);
            sample_for_lang.push(' ');
        }
        if let Some(ref b) = clean_pr_body {
            sample_for_lang.push_str(b);
            sample_for_lang.push(' ');
        }
        for c in &clean_commits {
            sample_for_lang.push_str(c);
            sample_for_lang.push(' ');
        }

        let target_lang = resolve_target_language(project_lang, &sample_for_lang);
        let system_prompt = get_system_prompt(target_lang);

        let user_prompt = build_user_prompt(
            target_lang,
            clean_pr_title.as_deref(),
            clean_pr_body.as_deref(),
            &clean_commits,
        );

        // 1. Aşama: NVIDIA NIM (NVIDIA_NIM_MODELS sırasına göre, toplam 45 sn sınırıyla)
        if let Some(ref api_key) = self.config.nvidia_nim_api_key {
            let chain_started = Instant::now();
            for model in &self.config.nvidia_nim_models {
                let chain_remaining = Duration::from_millis(NIM_CHAIN_TIMEOUT_MS)
                    .saturating_sub(chain_started.elapsed());
                if chain_remaining.is_zero() {
                    tracing::warn!(
                        chain_elapsed_ms = chain_started.elapsed().as_millis() as u64,
                        "NVIDIA NIM fallback zinciri 45 sn sınırına ulaştı"
                    );
                    break;
                }
                let request_timeout = Duration::from_millis(known_profile(model).timeout_ms)
                    .min(chain_remaining);
                match self
                    .call_nvidia_nim(api_key, model, system_prompt, &user_prompt, request_timeout)
                    .await
                {
                    Ok(draft) => {
                        tracing::info!("AI özeti başarıyla üretildi (NVIDIA NIM: {}, dil: {})", model, target_lang);
                        return draft;
                    }
                    Err(e) => {
                        tracing::warn!("NVIDIA NIM modeli ({}) başarısız: {}, sonrakine geçiliyor", model, e);
                    }
                }
            }
        }

        // 2. Aşama: Deterministik Kural Motoru (Zero-failure, çevrimdışı ve tam güvenli)
        tracing::info!("AI devrede değil veya yanıt vermedi, deterministik kural motoru çalıştırılıyor (dil: {})", target_lang);
        generate_deterministic_entry(
            target_lang,
            clean_pr_title.as_deref(),
            clean_pr_body.as_deref(),
            &clean_commits,
        )
    }

    /// Projenin seçtiği çalışma moduna (parse_mode) ve diline göre en uygun ayrıştırıcıyı ve motoru çalıştırır
    pub async fn summarize_for_project(
        &self,
        parse_mode: &str,
        project_language: Option<&str>,
        pr_title: Option<&str>,
        pr_body: Option<&str>,
        commit_messages: &[String],
        commit_shas: &[String],
        labels: &[String],
    ) -> Option<EntryDraft> {
        let mut sample_for_lang = String::new();
        if let Some(t) = pr_title {
            sample_for_lang.push_str(t);
            sample_for_lang.push(' ');
        }
        if let Some(b) = pr_body {
            sample_for_lang.push_str(b);
            sample_for_lang.push(' ');
        }
        for c in commit_messages {
            sample_for_lang.push_str(c);
            sample_for_lang.push(' ');
        }
        let target_lang = resolve_target_language(project_language, &sample_for_lang);

        match parse_mode {
            "conventional" => {
                Some(crate::llm::deterministic::generate_conventional_entry(target_lang, commit_messages))
            }
            "pr_centric" => {
                let default_title = if target_lang == "en" { "Release update" } else { "Sürüm Geliştirmesi" };
                let title = pr_title.unwrap_or(default_title);
                crate::llm::deterministic::generate_pr_centric_entry(target_lang, title, pr_body, labels)
            }
            "raw_git" => {
                Some(crate::llm::deterministic::generate_raw_git_entry(target_lang, commit_messages, commit_shas))
            }
            _ => { // "ai_editorial"
                Some(self.summarize(project_language, pr_title, pr_body, commit_messages).await)
            }
        }
    }

    async fn call_nvidia_nim(
        &self,
        api_key: &str,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        request_timeout: Duration,
    ) -> Result<EntryDraft, String> {
        let url = "https://integrate.api.nvidia.com/v1/chat/completions";
        let profile = known_profile(model);

        let mut body = json!({
            "model": profile.name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": profile.temperature,
            "max_tokens": profile.max_tokens,
            "stream": false
        });
        if let Some(top_p) = profile.top_p {
            body["top_p"] = json!(top_p);
        }
        merge_json(&mut body, &profile.extra);

        let deadline = Instant::now() + request_timeout;
        let mut attempt = 0;
        let mut resp = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{} modeli zaman aşımına uğradı", model));
            }
            let result = self
                .http
                .post(url)
                .bearer_auth(api_key)
                .json(&body)
                .timeout(remaining)
                .send()
                .await;
            match result {
                Ok(response) if [429, 503].contains(&response.status().as_u16()) => {
                    tracing::warn!(
                        model = %model,
                        status = %response.status(),
                        "NVIDIA NIM kapasite/rate-limit hatası; aynı model beklenmeden fallback sürüyor"
                    );
                    return Err(format!("NVIDIA NIM {} döndü", response.status()));
                }
                Ok(response)
                    if attempt == 0 && [502, 504].contains(&response.status().as_u16()) =>
                {
                    tracing::warn!(
                        model = %model,
                        status = %response.status(),
                        retry_delay_ms = 300,
                        "NVIDIA NIM geçici HTTP hatası; aynı model bir kez yeniden deneniyor"
                    );
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Ok(response) => break response,
                Err(error) if attempt == 0 => {
                    tracing::warn!(
                        model = %model,
                        error = %error,
                        retry_delay_ms = 300,
                        "NVIDIA NIM ağ hatası; aynı model bir kez yeniden deneniyor"
                    );
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Err(error) => return Err(format!("İstek hatası: {}", error)),
            }
        };

        while resp.status().as_u16() == 202 {
            let request_id = resp
                .headers()
                .get("NVCF-REQID")
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "NVIDIA NIM 202 yanıtında NVCF-REQID başlığı yok".to_string())?
                .to_string();
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{} modeli status polling zaman aşımına uğradı", model));
            }
            tokio::time::sleep(Duration::from_secs(1).min(remaining)).await;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{} modeli status polling zaman aşımına uğradı", model));
            }
            resp = self
                .http
                .get(format!("https://integrate.api.nvidia.com/v1/status/{request_id}"))
                .bearer_auth(api_key)
                .timeout(remaining)
                .send()
                .await
                .map_err(|error| format!("NVIDIA NIM status isteği başarısız: {}", error))?;
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {}: {}", status, text));
        }

        let parsed: ChatCompletionResponse = resp
            .json()
            .await
            .map_err(|e| format!("JSON ayrıştırma hatası: {}", e))?;

        let raw_content = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| "Boş model yanıtı".to_string())?;

        parse_draft_json(raw_content)
    }
}

fn merge_json(body: &mut Value, extra: &Value) {
    if let (Some(body_map), Some(extra_map)) = (body.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_map {
            body_map.insert(k.clone(), v.clone());
        }
    }
}

/// Modellerin metin içinde ürettiği düşünce/reasoning bloklarını (<think>...</think> vb.)
/// akışa ve sürüm günlüğü kaydına karışmaması için temizler.
fn strip_reasoning_blocks(raw: &str) -> String {
    let mut text = raw.to_string();

    for (start_tag, end_tag) in &[
        ("<think>", "</think>"),
        ("<thought>", "</thought>"),
        ("[THINK]", "[/THINK]"),
        ("<reasoning>", "</reasoning>"),
    ] {
        while let Some(start) = text.find(start_tag) {
            if let Some(end) = text[start..].find(end_tag) {
                let end_abs = start + end + end_tag.len();
                text.replace_range(start..end_abs, "");
            } else {
                break;
            }
        }
    }

    text.trim().to_string()
}

fn build_user_prompt(
    target_lang: &str,
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    commits: &[String],
) -> String {
    let is_en = target_lang == "en";
    let mut parts = Vec::new();
    if let Some(t) = pr_title {
        if is_en {
            parts.push(format!("PR Title: {}", t));
        } else {
            parts.push(format!("PR Başlığı: {}", t));
        }
    }
    if let Some(b) = pr_body {
        let short_b = b.lines().take(5).collect::<Vec<_>>().join("\n");
        if is_en {
            parts.push(format!("PR Description:\n{}", short_b));
        } else {
            parts.push(format!("PR Açıklaması:\n{}", short_b));
        }
    }

    let commit_list = commits
        .iter()
        .take(15)
        .map(|c| format!("- {}", c))
        .collect::<Vec<_>>()
        .join("\n");
    if is_en {
        parts.push(format!("Commit Messages:\n{}", commit_list));
    } else {
        parts.push(format!("Commit Mesajları:\n{}", commit_list));
    }

    parts.join("\n\n")
}

fn parse_draft_json(raw: &str) -> Result<EntryDraft, String> {
    let cleaned = strip_reasoning_blocks(raw);

    // JSON bloğunu bul: ilk { ile son } arası dilimleme
    let json_slice = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start < end {
            &cleaned[start..=end]
        } else {
            &cleaned
        }
    } else {
        cleaned.trim_matches(|c| c == '`' || c == ' ' || c == '\n' || c == '\r')
    };

    let draft: EntryDraft = serde_json::from_str(json_slice)
        .map_err(|e| format!("EntryDraft JSON parse hatası: {}. Ham metin: {}", e, cleaned))?;

    let valid_category = match draft.category.to_uppercase().as_str() {
        "NEW" => "NEW".to_string(),
        "FIX" => "FIX".to_string(),
        _ => "IMPROVEMENT".to_string(),
    };

    Ok(EntryDraft {
        category: valid_category,
        title: draft.title.chars().take(120).collect(),
        body: draft.body.chars().take(400).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_text_language("sepet donma problemi çözüldü"), "tr");
        assert_eq!(detect_text_language("kullanıcı profiline iki adımlı doğrulama eklendi"), "tr");
        assert_eq!(detect_text_language("fix divide by zero error in checkout"), "en");
        assert_eq!(detect_text_language("add dark theme support and navbar icons"), "en");
    }

    #[test]
    fn test_resolve_target_language() {
        assert_eq!(resolve_target_language(Some("en"), "sepet hatası düzeltildi"), "en");
        assert_eq!(resolve_target_language(Some("tr"), "fixed checkout bug"), "tr");
        assert_eq!(resolve_target_language(Some("auto"), "fixed checkout bug"), "en");
        assert_eq!(resolve_target_language(Some("auto"), "sepet hatası giderildi"), "tr");
    }

    #[test]
    fn test_parse_draft_with_thinking_blocks() {
        let raw = "<think>\nDüşünce süreci: burada yapılan analizler akışa gitmemeli.\n</think>\n{\n  \"category\": \"NEW\",\n  \"title\": \"Görsel Özelleştirme\",\n  \"body\": \"Yeni tema stilleri eklendi.\"\n}";
        let parsed = parse_draft_json(raw).expect("Ayrıştırma başarılı olmalı");
        assert_eq!(parsed.category, "NEW");
        assert_eq!(parsed.title, "Görsel Özelleştirme");
        assert_eq!(parsed.body, "Yeni tema stilleri eklendi.");
    }

    #[test]
    fn test_parse_draft_clean_json() {
        let raw = "{\n  \"category\": \"FIX\",\n  \"title\": \"Mobil Hata Giderimi\",\n  \"body\": \"Kritik bir görsel çökme sorunu düzeltildi.\"\n}";
        let parsed = parse_draft_json(raw).expect("Ayrıştırma başarılı olmalı");
        assert_eq!(parsed.category, "FIX");
        assert_eq!(parsed.title, "Mobil Hata Giderimi");
    }
}
