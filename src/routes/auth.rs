use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use chrono::{Duration, Utc};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::password::{
    generate_reset_token, hash_password, hash_token, verify_password,
};
use crate::auth::session::{
    create_session, destroy_all_user_sessions, destroy_session, extract_session_token,
    get_user_from_session, make_cookie_header, make_logout_cookie,
};
use crate::db::models::User;
use crate::db::{
    create_password_reset, create_user, find_user_by_email, find_valid_password_reset,
    mark_password_reset_used, update_user_password, update_user_profile,
};
use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GİRİŞ (LOGIN)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub redirect: Option<String>,
}

pub async fn login_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(token) = extract_session_token(&headers) {
        if get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref()).await?.is_some() {
            return Ok(Redirect::to("/dashboard").into_response());
        }
    }

    let tmpl = state.jinja.get_template("login.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! { app_url => state.config.app_url })
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

pub async fn login_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim().to_lowercase();
    let user_opt = find_user_by_email(&state.db, &email, state.config.token_encryption_key.as_deref()).await?;

    let valid = if let Some(ref u) = user_opt {
        verify_password(&form.password, &u.password_hash)
    } else {
        // Zamanlama saldırısını (timing attack) önlemek için dummy doğrulama
        let _ = verify_password("dummy", "$argon2id$v=19$m=19456,t=2,p=1$fake$fake");
        false
    };

    if !valid {
        let tmpl = state.jinja.get_template("login.html")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let html = tmpl.render(context! {
            error => "E-posta adresi veya şifre hatalı.",
            email => email,
            app_url => state.config.app_url
        }).map_err(|e| AppError::Internal(e.to_string()))?;

        return Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response());
    }

    let user = user_opt.unwrap();
    let session_token = create_session(&state.db, &user.id).await?;
    let cookie = make_cookie_header(&session_token, &state.config.app_url);

    let redirect_url = form.redirect.filter(|r| r.starts_with('/')).unwrap_or_else(|| "/dashboard".to_string());

    let mut resp = Redirect::to(&redirect_url).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| AppError::Internal(format!("Cookie hatası: {}", e)))?,
    );

    Ok(resp)
}

// ---------------------------------------------------------------------------
// KAYIT (REGISTER)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RegisterForm {
    pub name: Option<String>,
    pub email: String,
    pub password: String,
    pub password_confirm: String,
    pub accept_terms: Option<String>,
}

pub async fn register_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(token) = extract_session_token(&headers) {
        if get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref()).await?.is_some() {
            return Ok(Redirect::to("/dashboard").into_response());
        }
    }

    let tmpl = state.jinja.get_template("register.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! { app_url => state.config.app_url })
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

pub async fn register_submit(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim().to_lowercase();

    let render_error = |err: &'static str| -> Result<Response, AppError> {
        let tmpl = state.jinja.get_template("register.html")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let html = tmpl.render(context! {
            error => err,
            email => email,
            name => form.name,
            app_url => state.config.app_url
        }).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok((StatusCode::BAD_REQUEST, Html(html)).into_response())
    };

    if form.accept_terms.is_none() {
        return render_error("Kullanım Şartları ve Gizlilik Politikasını onaylamalısınız.");
    }

    if !email.contains('@') || !email.contains('.') {
        return render_error("Lütfen geçerli bir e-posta adresi girin.");
    }

    // Posta sunucusu (MX) ve geçerli domain kontrolü
    if let Err(err_msg) = crate::auth::email_validator::validate_email_mx(&email).await {
        return render_error(err_msg);
    }

    if form.password.len() < 8 {
        return render_error("Şifreniz en az 8 karakter uzunluğunda olmalıdır.");
    }

    if form.password != form.password_confirm {
        return render_error("Girdiğiniz şifreler birbiriyle eşleşmiyor.");
    }

    if find_user_by_email(&state.db, &email, state.config.token_encryption_key.as_deref()).await?.is_some() {
        return render_error("Bu e-posta adresi zaten kullanımda. Giriş yapmayı deneyin.");
    }

    let password_hash = hash_password(&form.password)?;
    let new_user = User {
        id: Uuid::new_v4().to_string(),
        email: email.clone(),
        email_hash: None,
        password_hash,
        name: form.name.filter(|n| !n.trim().is_empty()),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };

    create_user(&state.db, &new_user, state.config.token_encryption_key.as_deref()).await?;
    tracing::info!("Yeni kullanıcı kaydedildi: {}", email);

    // Otomatik oturum aç
    let session_token = create_session(&state.db, &new_user.id).await?;
    let cookie = make_cookie_header(&session_token, &state.config.app_url);

    let mut resp = Redirect::to("/dashboard").into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| AppError::Internal(format!("Cookie hatası: {}", e)))?,
    );

    Ok(resp)
}

// ---------------------------------------------------------------------------
// ÇIKIŞ (LOGOUT)
// ---------------------------------------------------------------------------

pub async fn logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if let Some(token) = extract_session_token(&headers) {
        let _ = destroy_session(&state.db, &token).await;
    }

    let cookie = make_logout_cookie();
    let mut resp = Redirect::to("/login").into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| AppError::Internal(format!("Cookie hatası: {}", e)))?,
    );

    Ok(resp)
}

// ---------------------------------------------------------------------------
// ŞİFREMİ UNUTTUM (FORGOT PASSWORD)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ForgotPasswordForm {
    pub email: String,
}

pub async fn forgot_password_page(
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    if !state.config.is_smtp_configured() {
        return Ok(Redirect::to("/login").into_response());
    }

    let tmpl = state.jinja.get_template("forgot_password.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! { app_url => state.config.app_url })
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

pub async fn forgot_password_submit(
    State(state): State<AppState>,
    Form(form): Form<ForgotPasswordForm>,
) -> Result<Response, AppError> {
    if !state.config.is_smtp_configured() {
        return Ok(Redirect::to("/login").into_response());
    }

    let email = form.email.trim().to_lowercase();
    let user_opt = find_user_by_email(&state.db, &email, state.config.token_encryption_key.as_deref()).await.ok().flatten();

    if let Some(user) = user_opt {
        tracing::info!("Şifre sıfırlama talebi alındı, kayıtlı hesap bulundu: [{}]", user.email);
        let (raw_token, token_hash) = generate_reset_token();
        let expires_at = (Utc::now() + Duration::hours(1)).to_rfc3339();
        let reset_id = Uuid::new_v4().to_string();

        let _ = create_password_reset(&state.db, &reset_id, &user.id, &token_hash, &expires_at).await;

        let reset_link = format!("{}/reset-password?token={}", state.config.app_url.trim_end_matches('/'), raw_token);

        let sent = crate::email::send_password_reset_email(&state.config, &user.email, &reset_link).await;
        if sent {
            tracing::info!("Şifre sıfırlama e-postası başarıyla gönderildi: [{}]", user.email);
        } else {
            tracing::warn!("SMTP üzerinden e-posta gönderilemedi. Yedek log kaydı [{}]: {}", user.email, reset_link);
        }
    } else {
        // Kullanıcı kuralı: O e-postayla sistemde kayıtlı hesap yoksa KESİNLİKLE mail gitmesin!
        tracing::warn!("Şifre sıfırlama talebi reddedildi: [{}] adresiyle kayıtlı kullanıcı bulunamadı, mail GÖNDERİLMEYECEK.", email);
    }

    // Kullanıcı sayma (enumeration) saldırısını önlemek için her durumda aynı başarılı mesaj
    let tmpl = state.jinja.get_template("forgot_password.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! {
        success => "Eğer bu e-posta adresi sistemimizde kayıtlıysa, şifre sıfırlama bağlantısı üretildi. Lütfen gelen kutunuzu kontrol edin.",
        app_url => state.config.app_url
    }).map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

// ---------------------------------------------------------------------------
// ŞİFRE SIFIRLAMA ONAYI (RESET PASSWORD)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ResetPasswordQuery {
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct ResetPasswordForm {
    pub token: String,
    pub password: String,
    pub password_confirm: String,
}

pub async fn reset_password_page(
    State(state): State<AppState>,
    Query(query): Query<ResetPasswordQuery>,
) -> Result<Response, AppError> {
    if !state.config.is_smtp_configured() {
        return Ok(Redirect::to("/login").into_response());
    }

    let raw_token = match query.token {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => {
            return Ok(Redirect::to("/login").into_response());
        }
    };

    let token_hash = hash_token(&raw_token);
    let reset_opt = find_valid_password_reset(&state.db, &token_hash).await?;

    if reset_opt.is_none() {
        let tmpl = state.jinja.get_template("forgot_password.html")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let html = tmpl.render(context! {
            error => "Şifre sıfırlama bağlantısı geçersiz veya süresi dolmuş. Lütfen yeniden talep edin.",
            app_url => state.config.app_url
        }).map_err(|e| AppError::Internal(e.to_string()))?;

        return Ok((StatusCode::BAD_REQUEST, Html(html)).into_response());
    }

    let tmpl = state.jinja.get_template("reset_password.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! {
        token => raw_token,
        app_url => state.config.app_url
    }).map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

pub async fn reset_password_submit(
    State(state): State<AppState>,
    Form(form): Form<ResetPasswordForm>,
) -> Result<Response, AppError> {
    if !state.config.is_smtp_configured() {
        return Ok(Redirect::to("/login").into_response());
    }

    let raw_token = form.token.trim();
    let token_hash = hash_token(raw_token);

    let reset = match find_valid_password_reset(&state.db, &token_hash).await? {
        Some(r) => r,
        None => {
            let tmpl = state.jinja.get_template("forgot_password.html")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let html = tmpl.render(context! {
                error => "Sıfırlama bağlantısının süresi dolmuş veya zaten kullanılmış.",
                app_url => state.config.app_url
            }).map_err(|e| AppError::Internal(e.to_string()))?;
            return Ok((StatusCode::BAD_REQUEST, Html(html)).into_response());
        }
    };

    if form.password.len() < 8 {
        let tmpl = state.jinja.get_template("reset_password.html")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let html = tmpl.render(context! {
            error => "Yeni şifreniz en az 8 karakter olmalıdır.",
            token => raw_token,
            app_url => state.config.app_url
        }).map_err(|e| AppError::Internal(e.to_string()))?;
        return Ok((StatusCode::BAD_REQUEST, Html(html)).into_response());
    }

    if form.password != form.password_confirm {
        let tmpl = state.jinja.get_template("reset_password.html")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let html = tmpl.render(context! {
            error => "Şifreler eşleşmiyor.",
            token => raw_token,
            app_url => state.config.app_url
        }).map_err(|e| AppError::Internal(e.to_string()))?;
        return Ok((StatusCode::BAD_REQUEST, Html(html)).into_response());
    }

    // Şifreyi güncelle
    let new_hash = hash_password(&form.password)?;
    update_user_password(&state.db, &reset.user_id, &new_hash).await?;

    // Token'ı kullanıldı olarak işaretle
    mark_password_reset_used(&state.db, &reset.id).await?;

    // Güvenlik: Eski tüm aktif oturumları kapat
    destroy_all_user_sessions(&state.db, &reset.user_id).await?;

    tracing::info!("Kullanıcı şifresi başarıyla yenilendi (user_id: {})", reset.user_id);

    // Giriş sayfasına başarı mesajıyla yönlendir
    let tmpl = state.jinja.get_template("login.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl.render(context! {
        success => "Şifreniz başarıyla güncellendi! Yeni şifrenizle giriş yapabilirsiniz.",
        app_url => state.config.app_url
    }).map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(html).into_response())
}

// ---------------------------------------------------------------------------
// PROFİL GÜNCELLEME (AD SOYAD)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct UpdateProfileForm {
    pub name: String,
}

pub async fn update_profile_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(form): axum::Json<UpdateProfileForm>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Ad alanı boş bırakılamaz.".to_string()));
    }
    if name.len() > 80 {
        return Err(AppError::BadRequest("Ad en fazla 80 karakter olabilir.".to_string()));
    }

    update_user_profile(&state.db, &user.id, name).await?;

    Ok(axum::Json(serde_json::json!({ "success": true, "name": name })))
}

// ---------------------------------------------------------------------------
// ŞİFRE DEĞİŞTİRME (OTURUM AÇIKKEN)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub new_password_confirm: String,
}

pub async fn change_password_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(form): axum::Json<ChangePasswordForm>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    if !verify_password(&form.current_password, &user.password_hash) {
        return Err(AppError::Unauthorized("Mevcut şifreniz hatalı.".to_string()));
    }

    if form.new_password.len() < 8 {
        return Err(AppError::BadRequest("Yeni şifreniz en az 8 karakter olmalıdır.".to_string()));
    }

    if form.new_password != form.new_password_confirm {
        return Err(AppError::BadRequest("Yeni şifreler birbiriyle eşleşmiyor.".to_string()));
    }

    let new_hash = hash_password(&form.new_password)?;
    update_user_password(&state.db, &user.id, &new_hash).await?;

    // Güvenlik: bu oturum dışındaki tüm oturumları kapat, mevcut oturumu koru
    destroy_all_user_sessions(&state.db, &user.id).await?;
    let new_session_token = create_session(&state.db, &user.id).await?;
    let cookie = make_cookie_header(&new_session_token, &state.config.app_url);

    let mut resp = axum::Json(serde_json::json!({ "success": true })).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| AppError::Internal(format!("Cookie hatası: {}", e)))?,
    );

    Ok(resp)
}

// ---------------------------------------------------------------------------
// KVKK UYUMLU HESAP VE VERİ İMHASI (DELETE ACCOUNT)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct DeleteAccountForm {
    pub password: String,
    pub confirm_text: String, // "HESABIMI SİL"
}

pub async fn delete_account_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<DeleteAccountForm>,
) -> Result<Response, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref())
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    if form.confirm_text.trim() != "HESABIMI SİL" {
        return Err(AppError::BadRequest("Lütfen onay kutusuna büyük harflerle 'HESABIMI SİL' yazın.".to_string()));
    }

    if !verify_password(&form.password, &user.password_hash) {
        return Err(AppError::Unauthorized("Girdiğiniz şifre hatalı. Hesap silinemedi.".to_string()));
    }

    // KVKK Kalıcı İmha & Yerel Ledger Kaydı
    crate::auth::erasure::purge_user(
        &state.db,
        &user.id,
        &user.email,
    )
    .await?;

    let cookie = make_logout_cookie();
    let mut resp = Redirect::to("/login?deleted=true").into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| AppError::Internal(format!("Cookie hatası: {}", e)))?,
    );

    Ok(resp)
}

