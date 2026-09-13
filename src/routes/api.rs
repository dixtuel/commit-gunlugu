use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::session::{extract_session_token, get_user_from_session};
use crate::crypto::token::{decrypt_token, encrypt_token_for_storage};
use crate::db::models::{Project, WidgetBrand, WidgetEntry, WidgetPayload};
use crate::db::{
    find_project_by_widget_key, list_all_projects, list_entries_for_project,
    list_projects_for_user, update_entry_status, upsert_project,
};
use crate::error::AppError;
use crate::state::AppState;

pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "commit-gunlugu",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Gömülebilir JavaScript widget'ının tükettiği herkese açık JSON uç noktası.
/// CORS izinleri açıktır, rate limit ve gizlilik korumalıdır.
pub async fn get_widget_data(
    State(state): State<AppState>,
    Path(widget_key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = if widget_key == "demo" || widget_key == "w_demo" {
        let p = if let Some(ref demo_key) = state.config.demo_widget_key {
            find_project_by_widget_key(&state.db, demo_key).await?
        } else {
            None
        };
        if let Some(proj) = p {
            proj
        } else {
            sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at ASC LIMIT 1")
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| AppError::NotFound("Demo projesi bulunamadı".to_string()))?
        }
    } else {
        find_project_by_widget_key(&state.db, &widget_key)
            .await?
            .ok_or_else(|| AppError::NotFound("Geçersiz widget anahtarı".to_string()))?
    };

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;

    let widget_entries: Vec<WidgetEntry> = entries
        .into_iter()
        .map(|e| WidgetEntry {
            id: e.id,
            category: e.category,
            title: e.title,
            body: e.body,
            author: e.author_username,
            published_at: e.published_at.unwrap_or(e.created_at),
        })
        .collect();

    let changelog_url = format!("{}/c/{}", state.config.app_url.trim_end_matches('/'), project.slug);

    let payload = WidgetPayload {
        brand: WidgetBrand {
            name: project.brand_name.unwrap_or(project.name),
            color: project.brand_color,
            logo_url: project.brand_logo_url,
        },
        changelog_url,
        entries: widget_entries,
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, s-maxage=60"),
    );

    Ok((headers, Json(payload)))
}

/// Gömülebilir JavaScript betiğini doğrudan sıfır önbellek (no-cache) garantisiyle sunar.
pub async fn get_widget_js_handler() -> impl IntoResponse {
    let content = tokio::fs::read_to_string("static/js/widget.js")
        .await
        .unwrap_or_else(|_| "// widget.js yüklenemedi".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );

    (headers, content)
}

pub async fn publish_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    update_entry_status(&state.db, &entry_id, "PUBLISHED").await?;
    Ok(Json(json!({ "success": true, "status": "PUBLISHED" })))
}

pub async fn dismiss_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    update_entry_status(&state.db, &entry_id, "DISMISSED").await?;
    Ok(Json(json!({ "success": true, "status": "DISMISSED" })))
}

pub async fn delete_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let deleted = crate::db::delete_entry(&state.db, &entry_id, &user.id).await?;
    if deleted {
        Ok(Json(json!({ "success": true, "message": "Kayıt silindi" })))
    } else {
        Err(AppError::NotFound("Kayıt bulunamadı veya yetkiniz yok".to_string()))
    }
}

pub async fn delete_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let deleted = crate::db::delete_project(&state.db, &project_id, &user.id).await?;
    if deleted {
        Ok(Json(json!({ "success": true, "message": "Proje silindi" })))
    } else {
        Err(AppError::NotFound("Proje bulunamadı veya yetkiniz yok".to_string()))
    }
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub github_repo_full_name: String,
    pub name: String,
    pub slug: Option<String>,
    pub brand_color: Option<String>,
    pub parse_mode: Option<String>,
    pub audience: Option<String>,
    pub template_style: Option<String>,
    pub is_private: Option<bool>,
    pub custom_github_token: Option<String>,
}

pub async fn create_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Proje eklemek için giriş yapmalısınız.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let is_private = payload.is_private.unwrap_or(false);
    let custom_token = payload
        .custom_github_token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| encrypt_token_for_storage(&t, state.config.token_encryption_key.as_deref()));

    if is_private && custom_token.is_none() {
        return Err(AppError::BadRequest(
            "Gizli (private) depolar için kişisel GitHub Access Token girilmesi zorunludur.".to_string(),
        ));
    }

    let slug = payload.slug.unwrap_or_else(|| {
        payload.github_repo_full_name.replace('/', "-").to_lowercase()
    });

    let raw_secret = format!("whsec_{}", Uuid::new_v4().simple());
    let encrypted_webhook_secret = encrypt_token_for_storage(
        &raw_secret,
        state.config.token_encryption_key.as_deref(),
    );

    let project = Project {
        id: Uuid::new_v4().to_string(),
        user_id: Some(user.id),
        github_repo_full_name: payload.github_repo_full_name,
        name: payload.name,
        slug,
        widget_key: format!("w_{}", Uuid::new_v4().simple()),
        brand_name: None,
        brand_color: payload.brand_color.unwrap_or_else(|| "#5b8a7a".to_string()),
        brand_logo_url: None,
        webhook_secret: encrypted_webhook_secret,
        parse_mode: payload.parse_mode.unwrap_or_else(|| "ai_editorial".to_string()),
        audience: payload.audience.unwrap_or_else(|| "end_user".to_string()),
        template_style: payload.template_style.unwrap_or_else(|| "standard".to_string()),
        is_private: if is_private { 1 } else { 0 },
        custom_github_token: custom_token,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    upsert_project(&state.db, &project).await?;

    Ok((StatusCode::CREATED, Json(project)))
}

#[derive(Deserialize)]
pub struct UpdateProjectSettingsRequest {
    pub name: Option<String>,
    pub brand_color: Option<String>,
    pub parse_mode: String,
    pub audience: String,
    pub template_style: String,
    pub is_private: Option<bool>,
    pub custom_github_token: Option<String>,
}

pub async fn update_project_settings_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<UpdateProjectSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let existing = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if existing.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeyi düzenleme yetkiniz yok".to_string()));
    }

    let name = payload.name.unwrap_or(existing.name);
    let brand_color = payload.brand_color.unwrap_or(existing.brand_color);

    let is_private_val = payload
        .is_private
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(existing.is_private);

    let custom_token_owned = payload
        .custom_github_token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| encrypt_token_for_storage(&t, state.config.token_encryption_key.as_deref()));

    let effective_custom_token = custom_token_owned
        .as_deref()
        .or(existing.custom_github_token.as_deref());

    if is_private_val == 1 && effective_custom_token.is_none() {
        return Err(AppError::BadRequest(
            "Gizli (private) depolar için kişisel GitHub Access Token tanımlı olmalıdır.".to_string(),
        ));
    }

    crate::db::update_project_full_settings(
        &state.db,
        &project_id,
        &user.id,
        &name,
        &brand_color,
        &payload.parse_mode,
        &payload.audience,
        &payload.template_style,
        is_private_val,
        effective_custom_token,
    )
    .await?;

    Ok(Json(json!({ "success": true, "message": "Proje ayarları güncellendi" })))
}

#[derive(Deserialize)]
pub struct CreateManualEntryRequest {
    pub category: String,
    pub title: String,
    pub body: String,
    pub status: Option<String>,
}

pub async fn create_manual_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<CreateManualEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let project = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if project.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeye kayıt ekleme yetkiniz yok".to_string()));
    }

    let status = payload.status.unwrap_or_else(|| "DRAFT".to_string());
    let published_at = if status == "PUBLISHED" {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    let author_username = user.name.clone().or_else(|| {
        Some(user.email.split('@').next().unwrap_or("user").to_string())
    });

    let entry = crate::db::models::Entry {
        id: Uuid::new_v4().to_string(),
        project_id,
        category: payload.category,
        title: payload.title,
        body: payload.body,
        status,
        ai_generated: 0,
        source_commit_shas: "[]".to_string(),
        source_pr_number: None,
        author_username,
        published_at,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    crate::db::insert_entry(&state.db, &entry).await?;

    Ok((StatusCode::CREATED, Json(entry)))
}

pub async fn list_projects_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers);
    let user = if let Some(ref t) = token {
        get_user_from_session(&state.db, t, state.config.token_encryption_key.as_deref()).await?
    } else {
        None
    };

    let projects = if let Some(u) = user {
        list_projects_for_user(&state.db, &u.id).await?
    } else {
        list_all_projects(&state.db).await?
    };

    Ok(Json(projects))
}

pub async fn sync_github_commits_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let project = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if project.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeyi senkronize etme yetkiniz yok".to_string()));
    }

    let repo = &project.github_repo_full_name;
    let url = format!("https://api.github.com/repos/{}/commits?per_page=10", repo);

    let mut req = reqwest::Client::builder()
        .user_agent("commit-gunlugu/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Internal(format!("İstemci hatası: {}", e)))?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    let effective_token = project.custom_github_token.as_deref().and_then(|t| {
        let decrypted = decrypt_token(t.trim(), state.config.token_encryption_key.as_deref());
        if decrypted.is_empty() { None } else { Some(decrypted) }
    });

    let bearer_token = match effective_token {
        Some(t) => Some(t),
        None => {
            if project.is_private == 1 {
                return Err(AppError::BadRequest(
                    "Bu repo gizli (private) olarak işaretlenmiş. GitHub API kısıtları nedeniyle özel deponuza erişebilmek için lütfen proje ayarlarından kendi GitHub Personal Access Token'ınızı (repo yetkili) tanımlayın.".to_string(),
                ));
            }
            state.config.github_token.clone()
        }
    };

    if let Some(ref gh_token) = bearer_token {
        req = req.bearer_auth(gh_token);
    }

    let res = req
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub API isteği başarısız: {}", e)))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::BadRequest(format!(
                "GitHub deposu bulunamadı (404). Repo adı '{}' hatalı olabilir ya da depo gizli (private) ise erişim izni olan bir GitHub Token tanımlanmamış olabilir.",
                repo
            )));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(AppError::BadRequest(format!(
                "GitHub API yetkilendirme hatası (HTTP {}). Tanımlanan token geçersiz, süresi dolmuş veya bu depoyu okuma yetkisine (repo scope) sahip değil.",
                status
            )));
        }
        return Err(AppError::Internal(format!(
            "GitHub API hata döndürdü (HTTP {}): {}",
            status, body
        )));
    }

    let commits_json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub yanıtı parse edilemedi: {}", e)))?;

    let commits = commits_json.as_array().ok_or_else(|| {
        AppError::Internal("GitHub geçerli bir commit listesi döndürmedi".to_string())
    })?;

    // Henüz veritabanında olmayan commit'leri filtrele
    let mut new_commits = Vec::new();
    for c in commits.iter().rev() {
        let sha = match c.get("sha").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => continue,
        };

        if !crate::db::commit_sha_exists(&state.db, &project.id, sha).await? {
            new_commits.push(c.clone());
        }
    }

    if new_commits.is_empty() {
        return Ok(Json(json!({
            "success": true,
            "imported_count": 0,
            "message": "Tüm commit'ler zaten güncel. Yeni aktarılacak commit bulunamadı."
        })));
    }

    let count = new_commits.len();
    let state_clone = state.clone();
    let project_clone = project.clone();

    // Arka planda AI özetleme ve kayıt yürüt (HTTP bağlantısını bekletme ve timeout önleme)
    tokio::spawn(async move {
        for c in new_commits {
            let sha = match c.get("sha").and_then(|s| s.as_str()) {
                Some(s) => s,
                None => continue,
            };

            let message = c
                .get("commit")
                .and_then(|cm| cm.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("");

            let author_name = c
                .get("author")
                .and_then(|a| a.get("login"))
                .and_then(|l| l.as_str())
                .or_else(|| {
                    c.get("commit")
                        .and_then(|cm| cm.get("author"))
                        .and_then(|ca| ca.get("name"))
                        .and_then(|n| n.as_str())
                });

            let first_line = message.lines().next().unwrap_or(message);
            let commit_messages = vec![first_line.to_string()];
            let commit_shas = vec![sha.to_string()];

            let draft = match state_clone.llm.summarize_for_project(
                &project_clone.parse_mode,
                None,
                None,
                &commit_messages,
                &commit_shas,
                &[],
            ).await {
                Some(d) => d,
                None => continue,
            };

            let entry = crate::db::models::Entry {
                id: Uuid::new_v4().to_string(),
                project_id: project_clone.id.clone(),
                category: draft.category,
                title: draft.title,
                body: draft.body,
                status: "PUBLISHED".to_string(),
                ai_generated: if project_clone.parse_mode == "ai_editorial" { 1 } else { 0 },
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: author_name.map(crate::sanitizer::sanitize_text),
                published_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            let _ = crate::db::insert_entry(&state_clone.db, &entry).await;
            tracing::info!("Arka plan GitHub commit sürüm notu yayına alındı: {}", entry.title);
        }
    });

    Ok(Json(json!({
        "success": true,
        "imported_count": count,
        "message": format!("{} yeni commit bulundu! Sürüm notları arka planda hazırlanıp yayına alınıyor...", count)
    })))
}

/// Projeye özel Webhook Secret'ı yeniden üretir ve AES-256-GCM ile veritabanına yazar.
pub async fn regenerate_webhook_secret_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let existing = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if existing.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeyi düzenleme yetkiniz yok".to_string()));
    }

    let new_raw_secret = format!("whsec_{}", Uuid::new_v4().simple());
    let encrypted = encrypt_token_for_storage(&new_raw_secret, state.config.token_encryption_key.as_deref());

    sqlx::query("UPDATE projects SET webhook_secret = ?, updated_at = ? WHERE id = ?")
        .bind(&encrypted)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&project_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(json!({
        "success": true,
        "webhook_secret": new_raw_secret,
        "message": "Webhook anahtarı başarıyla yenilendi."
    })))
}
