use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::Config;
use crate::llm::deterministic::{generate_deterministic_entry, EntryDraft};
use crate::sanitizer::sanitize_text;

const SYSTEM_PROMPT: &str = r#"Sen kıdemli bir teknik ürün editörüsün ("Seyir Defteri" editörü). Görevin; ham Git commit mesajlarını ve Pull Request verilerini, yazılım ürününü kullanan son kullanıcılar için anlaşılır, editoryal ve değer odaklı bir sürüm günlüğü (changelog) kaydına dönüştürmektir.

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

#[derive(Clone)]
struct ModelProfile {
    name: String,
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
    extra: Value,
}

fn known_profile(name: &str) -> ModelProfile {
    match name {
        "nvidia/nemotron-3.5-lightning-30b-a3b" => ModelProfile {
            name: name.to_string(),
            temperature: 0.6,
            top_p: 0.95,
            max_tokens: 1024,
            // reasoning_budget=0: Sonsuz düşünce (reasoning) döngüsünü kapatıp anında editoryal JSON üretmesini sağlar.
            extra: json!({ "reasoning_budget": 0 }),
        },
        "meta/muse-glimmer-30b" => ModelProfile {
            name: name.to_string(),
            temperature: 0.95,
            top_p: 1.0,
            max_tokens: 1024,
            extra: json!({
                "reasoning_effort": "low",
                "chat_template_kwargs": { "reasoning_strength": "low" }
            }),
        },
        "poolside/laguna-xs-2.1" => ModelProfile {
            name: name.to_string(),
            temperature: 0.8,
            top_p: 0.95,
            max_tokens: 1024,
            extra: json!({}),
        },
        "deepseek-ai/deepseek-v4-flash-0731" => ModelProfile {
            name: name.to_string(),
            temperature: 1.0,
            top_p: 0.95,
            max_tokens: 2048,
            extra: json!({
                "reasoning_effort": "none",
                "chat_template_kwargs": { "enable_thinking": false }
            }),
        },
        "google/gemma-4-31b-it" => ModelProfile {
            name: name.to_string(),
            temperature: 0.5,
            top_p: 1.0,
            max_tokens: 2048,
            extra: json!({
                "chat_template_kwargs": { "enable_thinking": false }
            }),
        },
        other => ModelProfile {
            name: other.to_string(),
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 1024,
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
    /// 1. NVIDIA NIM modelleri (sırayla denenir)
    /// 2. Deterministik kural motoru (Zero-failure)
    pub async fn summarize(
        &self,
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

        let user_prompt = build_user_prompt(
            clean_pr_title.as_deref(),
            clean_pr_body.as_deref(),
            &clean_commits,
        );

        // 1. Aşama: NVIDIA NIM (DeepSeek V4, Nemotron 3.5, Gemma 4)
        if let Some(ref api_key) = self.config.nvidia_nim_api_key {
            for model in &self.config.nvidia_nim_models {
                match self.call_nvidia_nim(api_key, model, &user_prompt).await {
                    Ok(draft) => {
                        tracing::info!("AI özeti başarıyla üretildi (NVIDIA NIM: {})", model);
                        return draft;
                    }
                    Err(e) => {
                        tracing::warn!("NVIDIA NIM modeli ({}) başarısız: {}, sonrakine geçiliyor", model, e);
                    }
                }
            }
        }

        // 2. Aşama: Deterministik Kural Motoru (Zero-failure, çevrimdışı ve tam güvenli)
        tracing::info!("AI devrede değil veya yanıt vermedi, deterministik kural motoru çalıştırılıyor");
        generate_deterministic_entry(
            clean_pr_title.as_deref(),
            clean_pr_body.as_deref(),
            &clean_commits,
        )
    }

    /// Projenin seçtiği çalışma moduna (parse_mode) göre en uygun ayrıştırıcıyı ve motoru çalıştırır
    pub async fn summarize_for_project(
        &self,
        parse_mode: &str,
        pr_title: Option<&str>,
        pr_body: Option<&str>,
        commit_messages: &[String],
        commit_shas: &[String],
        labels: &[String],
    ) -> Option<EntryDraft> {
        match parse_mode {
            "conventional" => {
                Some(crate::llm::deterministic::generate_conventional_entry(commit_messages))
            }
            "pr_centric" => {
                let title = pr_title.unwrap_or("Sürüm Geliştirmesi");
                crate::llm::deterministic::generate_pr_centric_entry(title, pr_body, labels)
            }
            "raw_git" => {
                Some(crate::llm::deterministic::generate_raw_git_entry(commit_messages, commit_shas))
            }
            _ => { // "ai_editorial"
                Some(self.summarize(pr_title, pr_body, commit_messages).await)
            }
        }
    }

    async fn call_nvidia_nim(
        &self,
        api_key: &str,
        model: &str,
        user_prompt: &str,
    ) -> Result<EntryDraft, String> {
        let url = "https://integrate.api.nvidia.com/v1/chat/completions";
        let profile = known_profile(model);

        let mut body = json!({
            "model": profile.name,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": profile.temperature,
            "top_p": profile.top_p,
            "max_tokens": profile.max_tokens,
            "stream": false
        });
        merge_json(&mut body, &profile.extra);

        let resp = self
            .http
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("İstek hatası: {}", e))?;

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
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    commits: &[String],
) -> String {
    let mut parts = Vec::new();
    if let Some(t) = pr_title {
        parts.push(format!("PR Başlığı: {}", t));
    }
    if let Some(b) = pr_body {
        let short_b = b.lines().take(5).collect::<Vec<_>>().join("\n");
        parts.push(format!("PR Açıklaması:\n{}", short_b));
    }

    let commit_list = commits
        .iter()
        .take(15)
        .map(|c| format!("- {}", c))
        .collect::<Vec<_>>()
        .join("\n");
    parts.push(format!("Commit Mesajları:\n{}", commit_list));

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
