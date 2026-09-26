use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::session::{extract_session_token, get_user_from_session};
use crate::crypto::token::{decrypt_token, encrypt_token_for_storage};
use crate::db::models::{Project, WidgetBrand, WidgetEntry, WidgetPayload};
use crate::db::{find_project_by_widget_key, list_all_projects, list_projects_for_user, update_entry_status, upsert_project};
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

#[derive(Debug, Deserialize, Default)]
pub struct WidgetQuery {
    pub branch: Option<String>,
}

/// Gömülebilir JavaScript widget'ının tükettiği herkese açık JSON uç noktası.
/// CORS izinleri açıktır, rate limit ve gizlilik korumalıdır.
pub async fn get_widget_data(
    State(state): State<AppState>,
    Path(widget_key): Path<String>,
    Query(query): Query<WidgetQuery>,
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

    let filter_branches = if let Some(ref b) = query.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        vec![b.to_string()]
    } else {
        project.tracked_branches()
    };

    let entries = crate::db::list_entries_for_project_branches(
        &state.db,
        &project.id,
        true,
        &filter_branches,
        state.config.token_encryption_key.as_deref(),
    )
    .await?;

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

#[derive(Debug, Deserialize)]
pub struct MultiWidgetQuery {
    pub keys: Option<String>,
    pub limit: Option<usize>,
    pub distinct: Option<bool>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiWidgetEntry {
    pub id: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub author: Option<String>,
    pub published_at: String,
    pub project_name: String,
    pub project_slug: String,
    pub brand_color: String,
    pub changelog_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiWidgetPayload {
    pub entries: Vec<MultiWidgetEntry>,
}

/// Birden fazla projenin sürüm notlarını harmanlayan çoklu proje widget JSON uç noktası.
/// `keys`: Virgülle ayrılmış proje widget anahtarları (ör. w_1,w_2,w_3).
/// `distinct`: true ise (varsayılan), her projeden en fazla 1 güncel kayıt seçerek çeşitlilik sağlar.
pub async fn get_multi_widget_data(
    State(state): State<AppState>,
    Query(query): Query<MultiWidgetQuery>,
) -> Result<impl IntoResponse, AppError> {
    let raw_keys = query.keys.unwrap_or_default();
    let keys: Vec<&str> = raw_keys.split(',').map(|k| k.trim()).filter(|k| !k.is_empty()).collect();
    if keys.is_empty() {
        return Err(AppError::BadRequest("En az bir widget anahtarı ('keys') belirtilmelidir.".to_string()));
    }

    let limit = query.limit.unwrap_or(3).clamp(1, 50);
    let distinct = query.distinct.unwrap_or(true);

    let mut resolved_projects = Vec::new();
    for k in keys {
        let proj = if k == "demo" || k == "w_demo" {
            if let Some(ref demo_key) = state.config.demo_widget_key {
                find_project_by_widget_key(&state.db, demo_key).await?
            } else {
                sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at ASC LIMIT 1")
                    .fetch_optional(&state.db)
                    .await?
            }
        } else {
            find_project_by_widget_key(&state.db, k).await?
        };

        if let Some(p) = proj {
            if !resolved_projects.iter().any(|existing: &Project| existing.id == p.id) {
                resolved_projects.push(p);
            }
        }
    }

    if resolved_projects.is_empty() {
        return Err(AppError::NotFound("Belirtilen anahtarlarla eşleşen proje bulunamadı.".to_string()));
    }

    let mut all_entries = Vec::new();
    let mut per_project_newest: Vec<MultiWidgetEntry> = Vec::new();

    for p in &resolved_projects {
        let filter_branches = if let Some(ref b) = query.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
            vec![b.to_string()]
        } else {
            p.tracked_branches()
        };
        let entries = crate::db::list_entries_for_project_branches(
            &state.db,
            &p.id,
            true,
            &filter_branches,
            state.config.token_encryption_key.as_deref(),
        )
        .await?;
        let changelog_url = format!("{}/c/{}", state.config.app_url.trim_end_matches('/'), p.slug);
        let project_name = p.brand_name.clone().unwrap_or_else(|| p.name.clone());

        let mapped_entries: Vec<MultiWidgetEntry> = entries
            .into_iter()
            .map(|e| MultiWidgetEntry {
                id: e.id,
                category: e.category,
                title: e.title,
                body: e.body,
                author: e.author_username,
                published_at: e.published_at.unwrap_or(e.created_at),
                project_name: project_name.clone(),
                project_slug: p.slug.clone(),
                brand_color: p.brand_color.clone(),
                changelog_url: changelog_url.clone(),
            })
            .collect();

        if let Some(newest) = mapped_entries.first() {
            per_project_newest.push(newest.clone());
        }
        all_entries.extend(mapped_entries);
    }

    let mut result_entries = Vec::new();

    if distinct {
        per_project_newest.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        for item in per_project_newest {
            result_entries.push(item);
            if result_entries.len() >= limit {
                break;
            }
        }

        if result_entries.len() < limit {
            all_entries.sort_by(|a, b| b.published_at.cmp(&a.published_at));
            for item in all_entries {
                if !result_entries.iter().any(|e| e.id == item.id) {
                    result_entries.push(item);
                    if result_entries.len() >= limit {
                        break;
                    }
                }
            }
        }
    } else {
        all_entries.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        result_entries = all_entries.into_iter().take(limit).collect();
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, s-maxage=60"),
    );

    Ok((headers, Json(MultiWidgetPayload { entries: result_entries })))
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
    pub language: Option<String>,
    pub tracked_branch: Option<String>,
    pub is_private: Option<bool>,
    pub custom_github_token: Option<String>,
}

fn normalize_tracked_branch(value: &str) -> Result<String, AppError> {
    let mut normalized = Vec::new();
    for raw in value.split(['\n', '\r', ',']).map(str::trim).filter(|line| !line.is_empty()) {
        let branch = raw.strip_prefix("refs/heads/").unwrap_or(raw);
        let invalid_component = branch
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.') || part.ends_with(".lock"));
        let invalid_character = branch.chars().any(|c| {
            c.is_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '|' | '"' | '<' | '>')
        });

        if branch.len() > 255
            || branch.is_empty()
            || branch.starts_with('-')
            || branch.starts_with('/')
            || branch.ends_with('/')
            || branch.ends_with('.')
            || branch.contains("..")
            || branch.contains("@{")
            || invalid_component
            || invalid_character
            || raw.starts_with("refs/") && !raw.starts_with("refs/heads/")
        {
            return Err(AppError::BadRequest(
                "Branch adı geçersiz. Yalnızca GitHub branch adını yazın (ör. main veya release/1.x).".to_string(),
            ));
        }

        if !normalized.iter().any(|selected| selected == branch) {
            normalized.push(branch.to_string());
            if normalized.len() > 100 {
                return Err(AppError::BadRequest("En fazla 100 branch seçebilirsiniz.".to_string()));
            }
        }
    }
    Ok(normalized.join("\n"))
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
    let tracked_branch = normalize_tracked_branch(payload.tracked_branch.as_deref().unwrap_or(""))?;
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
        language: payload.language.unwrap_or_else(|| "auto".to_string()),
        tracked_branch,
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
    pub language: Option<String>,
    pub tracked_branch: Option<String>,
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
    let language = payload.language.unwrap_or(existing.language);
    let tracked_branch = match payload.tracked_branch.as_deref() {
        Some(branch) => normalize_tracked_branch(branch)?,
        None => existing.tracked_branch,
    };

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
        &language,
        &tracked_branch,
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

    crate::db::insert_entry(
        &state.db,
        &entry,
        state.config.token_encryption_key.as_deref(),
    )
    .await?;
    for branch in project.tracked_branches() {
        crate::db::assign_entry_branch(&state.db, &entry.id, &entry.project_id, &branch).await?;
    }

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
    let client = reqwest::Client::builder()
        .user_agent("commit-gunlugu/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Internal(format!("İstemci hatası: {}", e)))?;

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

    let selected_branches = project.tracked_branches();
    let branches = if selected_branches.is_empty() { vec![String::new()] } else { selected_branches };
    let mut commits_by_sha: Vec<(String, serde_json::Value, Vec<String>)> = Vec::new();
    for branch in &branches {
        let url = format!("https://api.github.com/repos/{}/commits?per_page=10", repo);
        let mut req = client.get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if !branch.is_empty() { req = req.query(&[("sha", branch.as_str())]); }
        if let Some(ref gh_token) = bearer_token { req = req.bearer_auth(gh_token); }
        let res = req.send().await
            .map_err(|e| AppError::Internal(format!("GitHub API isteği başarısız: {}", e)))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(AppError::BadRequest(format!("GitHub deposu veya '{}' branch'i bulunamadı.", if branch.is_empty() { "varsayılan" } else { branch })));
            }
            if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                return Err(AppError::BadRequest(format!("GitHub API yetkilendirme hatası (HTTP {}). Token geçersiz, süresi dolmuş veya depoyu okuma yetkisi yok.", status)));
            }
            return Err(AppError::Internal(format!("GitHub API hata döndürdü (HTTP {}): {}", status, body)));
        }
        let commits_json: serde_json::Value = res.json().await
            .map_err(|e| AppError::Internal(format!("GitHub yanıtı parse edilemedi: {}", e)))?;
        let commits = commits_json.as_array().ok_or_else(|| AppError::Internal("GitHub geçerli bir commit listesi döndürmedi".to_string()))?;
        for commit in commits.iter().rev() {
            let Some(sha) = commit.get("sha").and_then(|value| value.as_str()) else { continue };
            let already_recorded = if branch.is_empty() {
                crate::db::commit_sha_exists(&state.db, &project.id, sha).await?
            } else {
                crate::db::assign_branch_to_existing_commit(&state.db, &project.id, sha, branch).await?
            };
            if already_recorded { continue; }
            if let Some((_, _, existing_branches)) = commits_by_sha.iter_mut().find(|(existing_sha, _, _)| existing_sha == sha) {
                if !branch.is_empty() && !existing_branches.contains(branch) { existing_branches.push(branch.clone()); }
            } else {
                commits_by_sha.push((sha.to_string(), commit.clone(), if branch.is_empty() { Vec::new() } else { vec![branch.clone()] }));
            }
        }
    }

    let new_commits = commits_by_sha;

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
        for (_, c, entry_branches) in new_commits {
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
                Some(&project_clone.language),
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

            let _ = crate::db::insert_entry(
                &state_clone.db,
                &entry,
                state_clone.config.token_encryption_key.as_deref(),
            )
            .await;
            for branch in entry_branches {
                let _ = crate::db::assign_entry_branch(&state_clone.db, &entry.id, &entry.project_id, &branch).await;
            }
            tracing::info!("Arka plan GitHub commit sürüm notu yayına alındı: {}", entry.title);
        }
    });

    Ok(Json(json!({
        "success": true,
        "imported_count": count,
        "message": format!("{} yeni commit bulundu! Sürüm notları arka planda hazırlanıp yayına alınıyor...", count)
    })))
}

pub async fn list_project_branches_handler(
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
        .await?.ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;
    if project.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projenin branch'lerini görme yetkiniz yok".to_string()));
    }

    let token = project.custom_github_token.as_deref().and_then(|stored| {
        let value = decrypt_token(stored.trim(), state.config.token_encryption_key.as_deref());
        if value.is_empty() { None } else { Some(value) }
    }).or_else(|| state.config.github_token.clone());
    if project.is_private == 1 && token.is_none() {
        return Err(AppError::BadRequest("Private repo branch'lerini listelemek için proje ayarlarında GitHub token gerekli.".to_string()));
    }

    let client = reqwest::Client::builder()
        .user_agent("commit-gunlugu/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| AppError::Internal(format!("İstemci hatası: {}", e)))?;
    let mut branches = Vec::new();
    for page in 1..=10 {
        let url = format!("https://api.github.com/repos/{}/branches", project.github_repo_full_name);
        let mut request = client.get(url)
            .query(&[("per_page", 100), ("page", page)])
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(ref value) = token { request = request.bearer_auth(value); }
        let response = request.send().await
            .map_err(|e| AppError::Internal(format!("GitHub branch isteği başarısız: {}", e)))?;
        if !response.status().is_success() {
            return Err(AppError::BadRequest(format!("GitHub branch listesi alınamadı (HTTP {}).", response.status())));
        }
        let page_items: Vec<serde_json::Value> = response.json().await
            .map_err(|e| AppError::Internal(format!("GitHub branch yanıtı parse edilemedi: {}", e)))?;
        let count = page_items.len();
        branches.extend(page_items.into_iter().filter_map(|item| item.get("name").and_then(|name| name.as_str()).map(str::to_string)));
        if count < 100 { break; }
    }
    Ok(Json(json!({ "branches": branches, "selected": project.tracked_branches() })))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_tracked_branch() {
        // Boş
        assert_eq!(normalize_tracked_branch("").unwrap(), "");
        assert_eq!(normalize_tracked_branch("   ").unwrap(), "");

        // Tek branch
        assert_eq!(normalize_tracked_branch("main").unwrap(), "main");
        assert_eq!(normalize_tracked_branch("refs/heads/main").unwrap(), "main");

        // Çoklu branch (satır satır)
        assert_eq!(
            normalize_tracked_branch("main\nbeta\nrelease/1.x").unwrap(),
            "main\nbeta\nrelease/1.x"
        );

        // Çoklu branch (virgüllü)
        assert_eq!(
            normalize_tracked_branch("main, beta, release/1.x").unwrap(),
            "main\nbeta\nrelease/1.x"
        );

        // Karışık (virgül + satır + fazladan boşluk + refs/heads/ öneki)
        assert_eq!(
            normalize_tracked_branch("refs/heads/main,   dev\n  release/v2  ").unwrap(),
            "main\ndev\nrelease/v2"
        );

        // Tekrarlı branch'lerin tekilleştirilmesi
        assert_eq!(
            normalize_tracked_branch("main, dev, main").unwrap(),
            "main\ndev"
        );

        // Geçersiz karakterler içeren branch
        assert!(normalize_tracked_branch("invalid branch with spaces").is_err());
        assert!(normalize_tracked_branch("bad..branch").is_err());
    }
}

