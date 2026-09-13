use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::db::{find_project_by_slug, list_entries_for_project};
use crate::error::AppError;
use crate::state::AppState;

/// Standart Markdown (CHANGELOG.md) formatında dışa aktarma (git-cliff / semantic-release stili)
pub async fn export_markdown_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("'{}' projesi bulunamadı", slug)))?;

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;

    let mut md = String::new();
    md.push_str(&format!("# {} — Sürüm Günlüğü (Changelog)\n\n", project.name));
    md.push_str(&format!("> Otomatik olarak [Commit Günlüğü]({}/c/{}) ile üretilmiştir.\n\n", state.config.app_url.trim_end_matches('/'), project.slug));

    if entries.is_empty() {
        md.push_str("*Henüz yayınlanmış bir sürüm notu bulunmuyor.*\n");
    } else {
        for entry in entries {
            let date = entry.published_at.as_deref().unwrap_or(&entry.created_at);
            let short_date = if date.len() >= 10 { &date[..10] } else { date };

            let badge = match entry.category.as_str() {
                "NEW" => "🚀 **[YENİ]**",
                "FIX" => "🐛 **[DÜZELTME]**",
                _ => "⚡ **[İYİLEŞTİRME]**",
            };

            md.push_str(&format!("### {} {}\n", badge, entry.title));
            md.push_str(&format!("*Tarih: {}*", short_date));

            if project.audience == "developer" {
                if let Some(author) = entry.author_username {
                    md.push_str(&format!(" &bull; *Yazar: @{}*", author));
                }
                if let Some(pr) = entry.source_pr_number {
                    md.push_str(&format!(" &bull; *PR: #{}\n*", pr));
                } else {
                    md.push('\n');
                }
            } else {
                md.push('\n');
            }

            md.push_str(&format!("\n{}\n\n---\n\n", entry.body));
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/markdown; charset=utf-8"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}-changelog.md\"", project.slug))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"changelog.md\"")),
    );

    Ok((headers, md))
}

/// Standart RSS 2.0 XML Akışı (RSS Okuyucular ve Takipçiler için)
pub async fn export_rss_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("'{}' projesi bulunamadı", slug)))?;

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;
    let project_url = format!("{}/c/{}", state.config.app_url.trim_end_matches('/'), project.slug);

    let mut rss = String::new();
    rss.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    rss.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n");
    rss.push_str("  <channel>\n");
    rss.push_str(&format!("    <title>{} — Sürüm Günlüğü</title>\n", escape_xml(&project.name)));
    rss.push_str(&format!("    <link>{}</link>\n", escape_xml(&project_url)));
    rss.push_str(&format!("    <description>{} için son sürüm notları ve değişiklikler.</description>\n", escape_xml(&project.name)));
    rss.push_str("    <language>tr</language>\n");
    rss.push_str(&format!("    <atom:link href=\"{}/feed.xml\" rel=\"self\" type=\"application/rss+xml\" />\n", escape_xml(&project_url)));

    for entry in entries {
        let entry_url = format!("{}#entry-{}", project_url, entry.id);
        rss.push_str("    <item>\n");
        rss.push_str(&format!("      <title>[{}] {}</title>\n", escape_xml(&entry.category), escape_xml(&entry.title)));
        rss.push_str(&format!("      <link>{}</link>\n", escape_xml(&entry_url)));
        rss.push_str(&format!("      <guid>{}</guid>\n", escape_xml(&entry_url)));
        rss.push_str(&format!("      <description><![CDATA[{}]]></description>\n", entry.body));
        rss.push_str(&format!("      <pubDate>{}</pubDate>\n", entry.published_at.unwrap_or(entry.created_at)));
        rss.push_str("    </item>\n");
    }

    rss.push_str("  </channel>\n");
    rss.push_str("</rss>\n");

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/rss+xml; charset=utf-8"));

    Ok((headers, rss))
}

/// JSON Feed v1 spesifikasyonu (API ve Otomasyon Entegrasyonları için)
pub async fn export_json_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("'{}' projesi bulunamadı", slug)))?;

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;
    let project_url = format!("{}/c/{}", state.config.app_url.trim_end_matches('/'), project.slug);

    let items: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id,
                "url": format!("{}#entry-{}", project_url, e.id),
                "title": e.title,
                "content_text": e.body,
                "category": e.category,
                "date_published": e.published_at.unwrap_or(e.created_at),
                "author": {
                    "name": e.author_username
                }
            })
        })
        .collect();

    let feed = json!({
        "version": "https://jsonfeed.org/version/1.1",
        "title": format!("{} — Sürüm Günlüğü", project.name),
        "home_page_url": project_url,
        "feed_url": format!("{}/feed.json", project_url),
        "items": items,
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json; charset=utf-8"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));

    Ok((headers, Json(feed)))
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
