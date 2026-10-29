use axum::{
    Json, Router,
    extract::{FromRequestParts, Query, State},
    http::{StatusCode, request::Parts},
    response::Redirect,
    routing::{get, patch, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_cookies::{
    Cookie, Cookies,
    cookie::{SameSite, time::Duration as CookieDuration},
};
use url::Url;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    db::now_ts,
    error::{ApiError, ApiResult},
};

const SESSION_COOKIE: &str = "coffee_session";
const OAUTH_STATE_COOKIE: &str = "coffee_oauth_state";
const GUEST_IDENTITY_COOKIE: &str = "coffee_guest";
const GUEST_IDENTITY_TTL_SECONDS: i64 = 60 * 60 * 24 * 400;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/gitlab/login", get(gitlab_login))
        .route("/auth/gitlab/callback", get(gitlab_callback))
        .route("/auth/authentik/login", get(authentik_login))
        .route("/auth/authentik/callback", get(authentik_callback))
        .route("/auth/guest", post(guest_login))
        .route("/auth/logout", post(logout))
        .route("/me", get(me).patch(update_settings))
        .route("/me/settings", patch(update_settings))
        .route("/me/guest-recovery-code", post(reset_guest_recovery_code))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentUser {
    pub id: String,
    pub user_type: String,
    pub gitlab_id: Option<i64>,
    pub display_name: String,
    pub leaderboard_opt_in: bool,
    pub achievement_sharing_opt_out: bool,
    pub mattermost_opt_in: bool,
    pub stock_manager: bool,
    pub paypal_link: Option<String>,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let header = parts
                .headers
                .get(axum::http::header::COOKIE)
                .and_then(|value| value.to_str().ok())
                .ok_or(ApiError::Unauthorized)?;
            let token = header
                .split(';')
                .filter_map(|part| Cookie::parse(part.trim().to_string()).ok())
                .find(|cookie| cookie.name() == SESSION_COOKIE)
                .map(|cookie| cookie.value().to_string())
                .ok_or(ApiError::Unauthorized)?;
            load_user_from_session(state, &token).await
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MeResponse {
    pub user: CurrentUser,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_recovery_code: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct GuestLoginRequest {
    pub display_name: String,
    pub recovery_code: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSettingsRequest {
    pub display_name: Option<String>,
    pub leaderboard_opt_in: Option<bool>,
    pub achievement_sharing_opt_out: Option<bool>,
    pub mattermost_opt_in: Option<bool>,
    pub paypal_link: Option<Option<String>>,
}

#[derive(Deserialize)]
pub(crate) struct OAuthCallback {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct GitLabTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GitLabUser {
    id: i64,
    name: String,
    email: Option<String>,
    public_email: Option<String>,
}

#[derive(Deserialize)]
struct GitLabGroup {
    path: Option<String>,
    full_path: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct GitLabEmail {
    email: String,
    confirmed_at: Option<String>,
}

#[derive(Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

#[derive(Deserialize)]
struct AuthentikTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct AuthentikUser {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    preferred_username: Option<String>,
    groups: Option<Vec<String>>,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/gitlab/login",
    responses((status = 302, description = "Redirects to GitLab OAuth"))
)]
pub(crate) async fn gitlab_login(
    State(state): State<AppState>,
    cookies: Cookies,
) -> ApiResult<Redirect> {
    if state.config.gitlab_oauth_client_id.is_empty() {
        return Err(ApiError::BadRequest(
            "GITLAB_OAUTH_CLIENT_ID is not configured".into(),
        ));
    }

    let oauth_state = random_token();
    cookies.add(cookie(
        OAUTH_STATE_COOKIE,
        oauth_state.clone(),
        600,
        secure_cookies(&state),
    ));

    let mut url = Url::parse(&format!("{}/oauth/authorize", state.config.gitlab_base_url))
        .map_err(|err| ApiError::Other(err.into()))?;
    url.query_pairs_mut()
        .append_pair("client_id", &state.config.gitlab_oauth_client_id)
        .append_pair("redirect_uri", &oauth_redirect_uri(&state, "gitlab"))
        .append_pair("response_type", "code")
        .append_pair("scope", &state.config.gitlab_oauth_scopes)
        .append_pair("state", &oauth_state);

    Ok(Redirect::temporary(url.as_str()))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/gitlab/callback",
    params(("code" = String, Query), ("state" = String, Query)),
    responses((status = 302, description = "Creates a session and redirects to app root"))
)]
pub(crate) async fn gitlab_callback(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(params): Query<OAuthCallback>,
) -> ApiResult<Redirect> {
    let expected_state = cookies
        .get(OAUTH_STATE_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state cookie".into()))?;
    if expected_state != params.state {
        return Err(ApiError::Forbidden);
    }

    let redirect_uri = oauth_redirect_uri(&state, "gitlab");
    let token_response = state
        .http
        .post(format!("{}/oauth/token", state.config.gitlab_base_url))
        .form(&[
            ("client_id", state.config.gitlab_oauth_client_id.as_str()),
            (
                "client_secret",
                state.config.gitlab_oauth_client_secret.as_str(),
            ),
            ("code", params.code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "GitLab OAuth token request failed");
            ApiError::Upstream
        })?;
    let token = gitlab_json::<GitLabTokenResponse>(token_response, "oauth token").await?;

    let user_response = state
        .http
        .get(format!("{}/api/v4/user", state.config.gitlab_base_url))
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "GitLab user request failed");
            ApiError::Upstream
        })?;
    let gitlab_user = gitlab_json::<GitLabUser>(user_response, "user profile").await?;

    let groups_response = state
        .http
        .get(format!("{}/api/v4/groups", state.config.gitlab_base_url))
        .query(&[("membership", "true"), ("per_page", "100")])
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "GitLab groups request failed");
            ApiError::Upstream
        })?;
    let groups = gitlab_json::<Vec<GitLabGroup>>(groups_response, "group membership").await?;

    if !gitlab_has_allowed_group(&groups, &state.config.auth_allowed_groups) {
        return Err(ApiError::Forbidden);
    }

    let gitlab_email = fetch_gitlab_email(&state, &token.access_token, &gitlab_user).await?;
    let stock_manager = gitlab_has_stock_manager_group(&groups, &state.config.auth_staff_groups);
    let user_id = upsert_provider_user(
        &state,
        ProviderUser {
            provider: "gitlab",
            subject: gitlab_user.id.to_string(),
            gitlab_id: Some(gitlab_user.id),
            email: gitlab_email.as_deref(),
            display_name: &gitlab_user.name,
            stock_manager,
        },
    )
    .await?;
    let session_token = create_session(&state, &user_id).await?;
    cookies.remove(Cookie::new(OAUTH_STATE_COOKIE, ""));
    cookies.add(cookie(
        SESSION_COOKIE,
        session_token,
        state.config.session_ttl.as_secs() as i64,
        secure_cookies(&state),
    ));

    Ok(Redirect::temporary(&state.config.frontend_base_url))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/authentik/login",
    responses((status = 302, description = "Redirects to Authentik OIDC"))
)]
pub(crate) async fn authentik_login(
    State(state): State<AppState>,
    cookies: Cookies,
) -> ApiResult<Redirect> {
    if state.config.authentik_issuer_url.is_empty()
        || state.config.authentik_oauth_client_id.is_empty()
    {
        return Err(ApiError::BadRequest(
            "AUTHENTIK_ISSUER_URL and AUTHENTIK_OAUTH_CLIENT_ID must be configured".into(),
        ));
    }

    let discovery = authentik_discovery(&state).await?;
    let oauth_state = random_token();
    cookies.add(cookie(
        OAUTH_STATE_COOKIE,
        oauth_state.clone(),
        600,
        secure_cookies(&state),
    ));

    let mut url =
        Url::parse(&discovery.authorization_endpoint).map_err(|err| ApiError::Other(err.into()))?;
    url.query_pairs_mut()
        .append_pair("client_id", &state.config.authentik_oauth_client_id)
        .append_pair("redirect_uri", &oauth_redirect_uri(&state, "authentik"))
        .append_pair("response_type", "code")
        .append_pair("scope", &state.config.authentik_oauth_scopes)
        .append_pair("state", &oauth_state);

    Ok(Redirect::temporary(url.as_str()))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/authentik/callback",
    params(("code" = String, Query), ("state" = String, Query)),
    responses((status = 302, description = "Creates a session and redirects to app root"))
)]
pub(crate) async fn authentik_callback(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(params): Query<OAuthCallback>,
) -> ApiResult<Redirect> {
    let expected_state = cookies
        .get(OAUTH_STATE_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state cookie".into()))?;
    if expected_state != params.state {
        return Err(ApiError::Forbidden);
    }

    let discovery = authentik_discovery(&state).await?;
    let redirect_uri = oauth_redirect_uri(&state, "authentik");
    let token_response = state
        .http
        .post(discovery.token_endpoint)
        .form(&[
            ("client_id", state.config.authentik_oauth_client_id.as_str()),
            (
                "client_secret",
                state.config.authentik_oauth_client_secret.as_str(),
            ),
            ("code", params.code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "Authentik OAuth token request failed");
            ApiError::Upstream
        })?;
    let token =
        provider_json::<AuthentikTokenResponse>(token_response, "Authentik", "oauth token").await?;

    let user_response = state
        .http
        .get(discovery.userinfo_endpoint)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "Authentik userinfo request failed");
            ApiError::Upstream
        })?;
    let authentik_user =
        provider_json::<AuthentikUser>(user_response, "Authentik", "userinfo").await?;
    if authentik_user.email_verified == Some(false) {
        return Err(ApiError::Forbidden);
    }
    let email = authentik_user
        .email
        .as_deref()
        .and_then(normalize_email)
        .ok_or_else(|| ApiError::BadRequest("Authentik email claim is required".into()))?;
    let groups = authentik_user.groups.unwrap_or_default();
    if !string_has_allowed_group(&groups, &state.config.auth_allowed_groups) {
        return Err(ApiError::Forbidden);
    }

    let display_name = authentik_user
        .name
        .as_deref()
        .and_then(non_empty_trimmed_str)
        .or(authentik_user.preferred_username.as_deref())
        .and_then(non_empty_trimmed_str)
        .unwrap_or(email.as_str());
    let user_id = upsert_provider_user(
        &state,
        ProviderUser {
            provider: "authentik",
            subject: authentik_user.sub,
            gitlab_id: None,
            email: verified_authentik_email(email.as_str(), authentik_user.email_verified),
            display_name,
            stock_manager: string_has_stock_manager_group(&groups, &state.config.auth_staff_groups),
        },
    )
    .await?;
    let session_token = create_session(&state, &user_id).await?;
    cookies.remove(Cookie::new(OAUTH_STATE_COOKIE, ""));
    cookies.add(cookie(
        SESSION_COOKIE,
        session_token,
        state.config.session_ttl.as_secs() as i64,
        secure_cookies(&state),
    ));

    Ok(Redirect::temporary(&state.config.frontend_base_url))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/guest",
    request_body = GuestLoginRequest,
    responses((status = 201, body = MeResponse))
)]
pub(crate) async fn guest_login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<GuestLoginRequest>,
) -> ApiResult<(StatusCode, Json<MeResponse>)> {
    let display_name = input.display_name.trim();
    if display_name.len() < 2 || display_name.len() > 80 {
        return Err(ApiError::BadRequest(
            "display_name must be between 2 and 80 characters".into(),
        ));
    }

    let recovery_code = input
        .recovery_code
        .as_deref()
        .and_then(normalize_recovery_code);
    if input
        .recovery_code
        .as_deref()
        .is_some_and(|code| !code.trim().is_empty())
        && recovery_code.is_none()
    {
        return Err(ApiError::BadRequest("recovery_code is not valid".into()));
    }

    let existing_guest_token = cookies
        .get(GUEST_IDENTITY_COOKIE)
        .map(|cookie| cookie.value().to_string());
    let guest_login = upsert_guest_user(
        &state,
        display_name,
        existing_guest_token.as_deref(),
        recovery_code.as_deref(),
    )
    .await?;

    let session_token = create_session(&state, &guest_login.user_id).await?;
    cookies.add(cookie(
        GUEST_IDENTITY_COOKIE,
        guest_login.guest_token,
        GUEST_IDENTITY_TTL_SECONDS,
        secure_cookies(&state),
    ));
    cookies.add(cookie(
        SESSION_COOKIE,
        session_token,
        state.config.session_ttl.as_secs() as i64,
        secure_cookies(&state),
    ));

    Ok((
        StatusCode::CREATED,
        Json(MeResponse {
            user: load_user_by_id(&state, &guest_login.user_id).await?,
            guest_recovery_code: guest_login.recovery_code,
        }),
    ))
}

struct GuestLoginResult {
    user_id: String,
    guest_token: String,
    recovery_code: Option<String>,
}

async fn upsert_guest_user(
    state: &AppState,
    display_name: &str,
    existing_guest_token: Option<&str>,
    recovery_code: Option<&str>,
) -> ApiResult<GuestLoginResult> {
    let now = now_ts();

    if let Some(code) = recovery_code {
        let recovery_hash = hash_guest_recovery(&state.config.session_secret, code);
        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT id, display_name FROM users WHERE user_type = 'guest' AND guest_recovery_hash = ?",
        )
        .bind(&recovery_hash)
        .fetch_optional(&state.db)
        .await?;

        if let Some((user_id, existing_display_name)) = existing {
            if !same_guest_display_name(&existing_display_name, display_name) {
                return Err(ApiError::BadRequest(
                    "recovery_code belongs to a different guest name".into(),
                ));
            }

            sqlx::query("UPDATE users SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(&user_id)
                .execute(&state.db)
                .await?;

            let guest_token =
                ensure_guest_identity(state, &user_id, existing_guest_token, now).await?;
            return Ok(GuestLoginResult {
                user_id,
                guest_token,
                recovery_code: Some(code.to_string()),
            });
        }

        return Err(ApiError::BadRequest(
            "recovery_code did not match a guest account".into(),
        ));
    }

    if let Some(token) = existing_guest_token {
        let guest_identity_hash = hash_guest_identity(&state.config.session_secret, token);
        let existing: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT u.id, u.display_name, u.guest_recovery_hash
             FROM guest_identities gi
             JOIN users u ON u.id = gi.user_id
             WHERE u.user_type = 'guest' AND gi.id_hash = ?",
        )
        .bind(&guest_identity_hash)
        .fetch_optional(&state.db)
        .await?;

        if let Some((user_id, existing_display_name, guest_recovery_hash)) = existing {
            if !same_guest_display_name(&existing_display_name, display_name) {
                return create_guest_user(state, display_name, now).await;
            }

            sqlx::query("UPDATE users SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(&user_id)
                .execute(&state.db)
                .await?;
            sqlx::query("UPDATE guest_identities SET last_used_at = ? WHERE id_hash = ?")
                .bind(now)
                .bind(&guest_identity_hash)
                .execute(&state.db)
                .await?;
            let generated_code = if guest_recovery_hash.is_none() {
                Some(set_guest_recovery_code(state, &user_id).await?)
            } else {
                None
            };
            return Ok(GuestLoginResult {
                user_id,
                guest_token: token.to_string(),
                recovery_code: generated_code,
            });
        }
    }

    create_guest_user(state, display_name, now).await
}

async fn create_guest_user(
    state: &AppState,
    display_name: &str,
    now: i64,
) -> ApiResult<GuestLoginResult> {
    let user_id = Uuid::new_v4().to_string();
    let guest_token = random_token();
    let recovery_code = random_guest_recovery_code();
    let recovery_hash = hash_guest_recovery(&state.config.session_secret, &recovery_code);
    sqlx::query(
        "INSERT INTO users
         (id, user_type, gitlab_id, guest_recovery_hash, display_name, leaderboard_opt_in,
          achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
            VALUES (?, 'guest', NULL, ?, ?, 0, 0, 0, 0, ?, ?)",
    )
    .bind(&user_id)
    .bind(&recovery_hash)
    .bind(display_name)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    insert_guest_identity(state, &user_id, &guest_token, now).await?;

    Ok(GuestLoginResult {
        user_id,
        guest_token,
        recovery_code: Some(recovery_code),
    })
}

fn same_guest_display_name(existing: &str, requested: &str) -> bool {
    existing.trim().eq_ignore_ascii_case(requested.trim())
}

async fn ensure_guest_identity(
    state: &AppState,
    user_id: &str,
    existing_guest_token: Option<&str>,
    now: i64,
) -> ApiResult<String> {
    if let Some(token) = existing_guest_token {
        let guest_identity_hash = hash_guest_identity(&state.config.session_secret, token);
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT user_id FROM guest_identities WHERE id_hash = ?")
                .bind(&guest_identity_hash)
                .fetch_optional(&state.db)
                .await?;
        if existing.as_ref().is_some_and(|(id,)| id == user_id) {
            sqlx::query("UPDATE guest_identities SET last_used_at = ? WHERE id_hash = ?")
                .bind(now)
                .bind(&guest_identity_hash)
                .execute(&state.db)
                .await?;
            return Ok(token.to_string());
        }
    }

    let guest_token = random_token();
    insert_guest_identity(state, user_id, &guest_token, now).await?;
    Ok(guest_token)
}

async fn insert_guest_identity(
    state: &AppState,
    user_id: &str,
    guest_token: &str,
    now: i64,
) -> ApiResult<()> {
    let guest_identity_hash = hash_guest_identity(&state.config.session_secret, guest_token);
    sqlx::query(
        "INSERT INTO guest_identities (id_hash, user_id, created_at, last_used_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(guest_identity_hash)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn ensure_guest_recovery_code(
    state: &AppState,
    user: &CurrentUser,
) -> ApiResult<Option<String>> {
    if user.user_type != "guest" {
        return Ok(None);
    }
    let (guest_recovery_hash,): (Option<String>,) =
        sqlx::query_as("SELECT guest_recovery_hash FROM users WHERE id = ?")
            .bind(&user.id)
            .fetch_one(&state.db)
            .await?;
    if guest_recovery_hash.is_some() {
        return Ok(None);
    }
    Ok(Some(set_guest_recovery_code(state, &user.id).await?))
}

async fn set_guest_recovery_code(state: &AppState, user_id: &str) -> ApiResult<String> {
    let recovery_code = random_guest_recovery_code();
    let recovery_hash = hash_guest_recovery(&state.config.session_secret, &recovery_code);
    sqlx::query("UPDATE users SET guest_recovery_hash = ?, updated_at = ? WHERE id = ?")
        .bind(recovery_hash)
        .bind(now_ts())
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(recovery_code)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses((status = 204, description = "Session terminated"))
)]
pub(crate) async fn logout(
    State(state): State<AppState>,
    cookies: Cookies,
) -> ApiResult<StatusCode> {
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        let hash = hash_session(&state.config.session_secret, cookie.value());
        sqlx::query("UPDATE sessions SET revoked_at = ? WHERE id_hash = ?")
            .bind(now_ts())
            .bind(hash)
            .execute(&state.db)
            .await?;
    }
    cookies.remove(Cookie::new(SESSION_COOKIE, ""));
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/me",
    responses((status = 200, body = MeResponse))
)]
pub(crate) async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<MeResponse>> {
    let guest_recovery_code = ensure_guest_recovery_code(&state, &user).await?;
    Ok(Json(MeResponse {
        user,
        guest_recovery_code,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/me/guest-recovery-code",
    responses((status = 200, body = MeResponse))
)]
pub(crate) async fn reset_guest_recovery_code(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<MeResponse>> {
    if user.user_type != "guest" {
        return Err(ApiError::Forbidden);
    }
    let guest_recovery_code = set_guest_recovery_code(&state, &user.id).await?;
    Ok(Json(MeResponse {
        user: load_user_by_id(&state, &user.id).await?,
        guest_recovery_code: Some(guest_recovery_code),
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/me/settings",
    request_body = UpdateSettingsRequest,
    responses((status = 200, body = MeResponse))
)]
pub(crate) async fn update_settings(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<UpdateSettingsRequest>,
) -> ApiResult<Json<MeResponse>> {
    if let Some(display_name) = &input.display_name {
        let trimmed = display_name.trim();
        if trimmed.len() < 2 || trimmed.len() > 80 {
            return Err(ApiError::BadRequest(
                "display_name must be between 2 and 80 characters".into(),
            ));
        }
        sqlx::query("UPDATE users SET display_name = ?, updated_at = ? WHERE id = ?")
            .bind(trimmed)
            .bind(now_ts())
            .bind(&user.id)
            .execute(&state.db)
            .await?;
    }

    if !user.stock_manager {
        sqlx::query("UPDATE users SET mattermost_opt_in = 0, updated_at = ? WHERE id = ?")
            .bind(now_ts())
            .bind(&user.id)
            .execute(&state.db)
            .await?;
    }

    let setting_updates: [(&'static str, Option<bool>, bool); 3] = [
        (
            "UPDATE users SET leaderboard_opt_in = ?, updated_at = ? WHERE id = ?",
            input.leaderboard_opt_in,
            false,
        ),
        (
            "UPDATE users SET achievement_sharing_opt_out = ?, updated_at = ? WHERE id = ?",
            input.achievement_sharing_opt_out,
            false,
        ),
        (
            "UPDATE users SET mattermost_opt_in = ?, updated_at = ? WHERE id = ?",
            input.mattermost_opt_in,
            true,
        ),
    ];
    for (sql, value, requires_stock_manager) in setting_updates {
        if requires_stock_manager && !user.stock_manager {
            continue;
        }
        if let Some(value) = value {
            sqlx::query(sql)
                .bind(value as i64)
                .bind(now_ts())
                .bind(&user.id)
                .execute(&state.db)
                .await?;
        }
    }

    // paypal_link: allow stock_manager to set or clear their paypal link
    if let Some(opt) = input.paypal_link {
        if !user.stock_manager {
            return Err(ApiError::Forbidden);
        } else {
            match opt {
                Some(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        sqlx::query(
                            "UPDATE users SET paypal_link = NULL, updated_at = ? WHERE id = ?",
                        )
                        .bind(now_ts())
                        .bind(&user.id)
                        .execute(&state.db)
                        .await?;
                    } else {
                        validate_payment_link(trimmed)?;
                        sqlx::query(
                            "UPDATE users SET paypal_link = ?, updated_at = ? WHERE id = ?",
                        )
                        .bind(trimmed)
                        .bind(now_ts())
                        .bind(&user.id)
                        .execute(&state.db)
                        .await?;
                    }
                }
                None => {
                    // explicitly clear
                    sqlx::query("UPDATE users SET paypal_link = NULL, updated_at = ? WHERE id = ?")
                        .bind(now_ts())
                        .bind(&user.id)
                        .execute(&state.db)
                        .await?;
                }
            }
        }
    }

    Ok(Json(MeResponse {
        user: load_user_by_id(&state, &user.id).await?,
        guest_recovery_code: None,
    }))
}

fn validate_payment_link(value: &str) -> ApiResult<()> {
    let url = url::Url::parse(value)
        .map_err(|_| ApiError::BadRequest("paypal_link must be a valid https URL".into()))?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(ApiError::BadRequest(
            "paypal_link must be a valid https URL".into(),
        ));
    }
    Ok(())
}

pub async fn load_user_by_id(state: &AppState, user_id: &str) -> ApiResult<CurrentUser> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, user_type, gitlab_id, display_name, leaderboard_opt_in,
         achievement_sharing_opt_out, mattermost_opt_in, stock_manager, paypal_link
            FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(row.into())
}

async fn load_user_from_session(state: &AppState, token: &str) -> ApiResult<CurrentUser> {
    let hash = hash_session(&state.config.session_secret, token);
    let now = now_ts();
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.user_type, u.gitlab_id, u.display_name, u.leaderboard_opt_in,
         u.achievement_sharing_opt_out, u.mattermost_opt_in, u.stock_manager, u.paypal_link
            FROM sessions s
            JOIN users u ON u.id = s.user_id
            WHERE s.id_hash = ? AND s.revoked_at IS NULL AND s.expires_at > ?",
    )
    .bind(hash)
    .bind(now)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(row.into())
}

struct ProviderUser<'a> {
    provider: &'static str,
    subject: String,
    gitlab_id: Option<i64>,
    email: Option<&'a str>,
    display_name: &'a str,
    stock_manager: bool,
}

async fn upsert_provider_user(state: &AppState, user: ProviderUser<'_>) -> ApiResult<String> {
    let now = now_ts();
    let email = user.email.and_then(normalize_email);
    let existing = find_provider_user(
        state,
        user.provider,
        &user.subject,
        user.gitlab_id,
        email.as_deref(),
    )
    .await?;

    if let Some((id,)) = existing {
        update_provider_user(state, &id, &user, email.as_deref(), now).await?;
        upsert_auth_identity(state, user.provider, &user.subject, &id, now).await?;
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users
         (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
          achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
            VALUES (?, 'member', ?, ?, ?, ?, 0, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user.gitlab_id)
    .bind(email.as_deref())
    .bind(user.display_name.trim())
    .bind(user.stock_manager as i64)
    .bind(user.stock_manager as i64)
    .bind(user.stock_manager as i64)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    upsert_auth_identity(state, user.provider, &user.subject, &id, now).await?;
    Ok(id)
}

async fn find_provider_user(
    state: &AppState,
    provider: &str,
    subject: &str,
    gitlab_id: Option<i64>,
    email: Option<&str>,
) -> ApiResult<Option<(String,)>> {
    if let Some(existing) =
        sqlx::query_as("SELECT user_id FROM auth_identities WHERE provider = ? AND subject = ?")
            .bind(provider)
            .bind(subject)
            .fetch_optional(&state.db)
            .await?
    {
        return Ok(Some(existing));
    }

    if let Some(gitlab_id) = gitlab_id {
        if let Some(existing) = sqlx::query_as("SELECT id FROM users WHERE gitlab_id = ?")
            .bind(gitlab_id)
            .fetch_optional(&state.db)
            .await?
        {
            return Ok(Some(existing));
        }
    }

    if let Some(email) = email {
        if let Some(existing) = sqlx::query_as("SELECT id FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&state.db)
            .await?
        {
            return Ok(Some(existing));
        }
    }

    Ok(None)
}

async fn update_provider_user(
    state: &AppState,
    id: &str,
    user: &ProviderUser<'_>,
    email: Option<&str>,
    now: i64,
) -> ApiResult<()> {
    let email_owner: Option<(String,)> = if let Some(email) = email {
        sqlx::query_as("SELECT id FROM users WHERE email = ? AND id != ?")
            .bind(email)
            .bind(id)
            .fetch_optional(&state.db)
            .await?
    } else {
        None
    };
    let email_for_update = email.filter(|_| email_owner.is_none());

    sqlx::query(
        "UPDATE users
         SET user_type = 'member',
             gitlab_id = COALESCE(?, gitlab_id),
             email = COALESCE(?, email),
             display_name = ?,
             stock_manager = ?,
             updated_at = ?
         WHERE id = ?",
    )
    .bind(user.gitlab_id)
    .bind(email_for_update)
    .bind(user.display_name.trim())
    .bind(user.stock_manager as i64)
    .bind(now)
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn upsert_auth_identity(
    state: &AppState,
    provider: &str,
    subject: &str,
    user_id: &str,
    now: i64,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO auth_identities (provider, subject, user_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(provider, subject)
         DO UPDATE SET user_id = excluded.user_id, updated_at = excluded.updated_at",
    )
    .bind(provider)
    .bind(subject)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn authentik_discovery(state: &AppState) -> ApiResult<OidcDiscovery> {
    let response = state
        .http
        .get(format!(
            "{}/.well-known/openid-configuration",
            state.config.authentik_issuer_url
        ))
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(?err, "Authentik discovery request failed");
            ApiError::Upstream
        })?;
    provider_json(response, "Authentik", "discovery").await
}

async fn fetch_gitlab_email(
    state: &AppState,
    access_token: &str,
    user: &GitLabUser,
) -> ApiResult<Option<String>> {
    if let Some(email) = gitlab_profile_email(user) {
        return Ok(Some(email));
    }

    let response = match state
        .http
        .get(format!(
            "{}/api/v4/user/emails",
            state.config.gitlab_base_url
        ))
        .bearer_auth(access_token)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!(?err, "GitLab user emails request failed");
            return Ok(None);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".into());
        tracing::warn!(
            status = %status,
            body = %truncate_for_log(&body),
            "GitLab user emails response was not successful"
        );
        return Ok(None);
    }

    let emails = response.json::<Vec<GitLabEmail>>().await.map_err(|err| {
        tracing::warn!(?err, "failed to decode GitLab user emails response");
        ApiError::Upstream
    })?;
    Ok(gitlab_email_from_emails(&emails))
}

fn gitlab_profile_email(user: &GitLabUser) -> Option<String> {
    user.email
        .as_deref()
        .and_then(normalize_email)
        .or_else(|| user.public_email.as_deref().and_then(normalize_email))
}

fn gitlab_email_from_emails(emails: &[GitLabEmail]) -> Option<String> {
    emails
        .iter()
        .find(|email| email.confirmed_at.is_some())
        .or_else(|| emails.first())
        .and_then(|email| normalize_email(&email.email))
}

async fn gitlab_json<T: DeserializeOwned>(
    response: reqwest::Response,
    context: &'static str,
) -> ApiResult<T> {
    provider_json(response, "GitLab", context).await
}

async fn provider_json<T: DeserializeOwned>(
    response: reqwest::Response,
    provider: &'static str,
    context: &'static str,
) -> ApiResult<T> {
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".into());
        tracing::warn!(
            status = %status,
            body = %truncate_for_log(&body),
            "{provider} {context} response was not successful"
        );
        return Err(ApiError::Upstream);
    }

    response.json::<T>().await.map_err(|err| {
        tracing::warn!(?err, "failed to decode {provider} {context} response");
        ApiError::Upstream
    })
}

fn truncate_for_log(value: &str) -> String {
    const LIMIT: usize = 500;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    format!("{}...", &value[..LIMIT])
}

async fn create_session(state: &AppState, user_id: &str) -> ApiResult<String> {
    let token = random_token();
    let now = now_ts();
    let id_hash = hash_session(&state.config.session_secret, &token);
    sqlx::query(
        "INSERT INTO sessions (id_hash, user_id, expires_at, revoked_at, created_at)
         VALUES (?, ?, ?, NULL, ?)",
    )
    .bind(id_hash)
    .bind(user_id)
    .bind(now + state.config.session_ttl.as_secs() as i64)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(token)
}

fn oauth_redirect_uri(state: &AppState, provider: &str) -> String {
    format!(
        "{}/api/v1/auth/{provider}/callback",
        state.config.public_base_url
    )
}

fn gitlab_has_allowed_group(groups: &[GitLabGroup], allowed: &[String]) -> bool {
    allowed
        .iter()
        .any(|allowed| gitlab_has_group(groups, allowed))
}

fn gitlab_has_group(groups: &[GitLabGroup], expected: &str) -> bool {
    groups.iter().any(|group| {
        [
            group.path.as_deref(),
            group.full_path.as_deref(),
            group.name.as_deref(),
        ]
        .iter()
        .flatten()
        .any(|value| value == &expected)
    })
}

fn gitlab_has_stock_manager_group(groups: &[GitLabGroup], staff_groups: &[String]) -> bool {
    groups.iter().any(|group| {
        [
            group.path.as_deref(),
            group.full_path.as_deref(),
            group.name.as_deref(),
        ]
        .iter()
        .flatten()
        .any(|value| is_stock_manager_group_name(value, staff_groups))
    })
}

fn string_has_allowed_group(groups: &[String], allowed: &[String]) -> bool {
    allowed
        .iter()
        .any(|allowed| string_has_group(groups, allowed))
}

fn string_has_group(groups: &[String], expected: &str) -> bool {
    groups.iter().any(|group| group == expected)
}

fn string_has_stock_manager_group(groups: &[String], staff_groups: &[String]) -> bool {
    groups
        .iter()
        .any(|group| is_stock_manager_group_name(group, staff_groups))
}

fn is_stock_manager_group_name(value: &str, staff_groups: &[String]) -> bool {
    let normalized = normalize_group_name(value);
    staff_groups
        .iter()
        .any(|group| normalize_group_name(group) == normalized)
}

fn normalize_group_name(value: &str) -> String {
    value
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn secure_cookies(state: &AppState) -> bool {
    state.config.public_base_url.starts_with("https://")
}

fn cookie(
    name: &'static str,
    value: String,
    max_age_seconds: i64,
    secure: bool,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(max_age_seconds))
        .build()
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn random_guest_recovery_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0_u8; 12];
    rand::fill(&mut bytes);
    bytes
        .iter()
        .map(|byte| ALPHABET[*byte as usize % ALPHABET.len()] as char)
        .collect()
}

fn normalize_recovery_code(code: &str) -> Option<String> {
    let normalized: String = code
        .chars()
        .filter(|value| !value.is_ascii_whitespace() && *value != '-')
        .map(|value| value.to_ascii_uppercase())
        .collect();
    const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    if normalized.len() == 12 && normalized.chars().all(|value| ALPHABET.contains(value)) {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_email(email: &str) -> Option<String> {
    let normalized = email.trim().to_ascii_lowercase();
    (normalized.contains('@') && normalized.len() <= 254).then_some(normalized)
}

fn verified_authentik_email(email: &str, email_verified: Option<bool>) -> Option<&str> {
    (email_verified == Some(true)).then_some(email)
}

fn non_empty_trimmed_str(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn hash_session(secret: &str, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn hash_guest_identity(secret: &str, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(b"guest");
    hasher.update(b":");
    hasher.update(token.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn hash_guest_recovery(secret: &str, recovery_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(b"guest-recovery");
    hasher.update(b":");
    hasher.update(recovery_code.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    user_type: String,
    gitlab_id: Option<i64>,
    display_name: String,
    leaderboard_opt_in: i64,
    achievement_sharing_opt_out: i64,
    mattermost_opt_in: i64,
    stock_manager: i64,
    paypal_link: Option<String>,
}

impl From<UserRow> for CurrentUser {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            user_type: row.user_type,
            gitlab_id: row.gitlab_id,
            display_name: row.display_name,
            leaderboard_opt_in: row.leaderboard_opt_in != 0,
            achievement_sharing_opt_out: row.achievement_sharing_opt_out != 0,
            mattermost_opt_in: row.mattermost_opt_in != 0,
            stock_manager: row.stock_manager != 0,
            paypal_link: row.paypal_link,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, db::connect};

    #[test]
    fn allowed_group_matches_path_or_full_path() {
        let groups = vec![GitLabGroup {
            path: Some("foo".into()),
            full_path: Some("coffee-admins".into()),
            name: None,
        }];
        assert!(gitlab_has_allowed_group(&groups, &["coffee-admins".into()]));
        assert!(!gitlab_has_allowed_group(&groups, &["other".into()]));
        assert!(gitlab_has_group(&groups, "coffee-admins"));
    }

    #[test]
    fn allowed_group_matches_authentik_group_claim() {
        let groups = vec!["coffee-admins".to_string(), "other".to_string()];
        assert!(string_has_allowed_group(&groups, &["coffee-admins".into()]));
        assert!(!string_has_allowed_group(&groups, &["coffee-users".into()]));
        assert!(string_has_group(&groups, "coffee-admins"));
    }

    #[test]
    fn stock_manager_group_accepts_gitlab_and_authentik_names() {
        let configured = vec!["coffee-admins".into()];
        assert!(string_has_stock_manager_group(
            &["coffeeadmins".into()],
            &configured,
        ));
        assert!(string_has_stock_manager_group(
            &["Coffee Admins".into()],
            &configured,
        ));
        assert!(!string_has_stock_manager_group(
            &["coffee-users".into()],
            &configured,
        ));

        let groups = vec![GitLabGroup {
            path: Some("coffee-admins".into()),
            full_path: None,
            name: None,
        }];
        assert!(gitlab_has_stock_manager_group(&groups, &configured));
    }

    #[test]
    fn gitlab_profile_email_prefers_private_then_public_email() {
        let user = GitLabUser {
            id: 1,
            name: "Ada".into(),
            email: Some(" ADA@EXAMPLE.ORG ".into()),
            public_email: Some("public@example.org".into()),
        };
        assert_eq!(gitlab_profile_email(&user), Some("ada@example.org".into()));

        let user = GitLabUser {
            id: 1,
            name: "Ada".into(),
            email: None,
            public_email: Some(" PUBLIC@EXAMPLE.ORG ".into()),
        };
        assert_eq!(
            gitlab_profile_email(&user),
            Some("public@example.org".into())
        );
    }

    #[test]
    fn gitlab_email_endpoint_prefers_confirmed_email() {
        let emails = vec![
            GitLabEmail {
                email: "unconfirmed@example.org".into(),
                confirmed_at: None,
            },
            GitLabEmail {
                email: " CONFIRMED@EXAMPLE.ORG ".into(),
                confirmed_at: Some("2026-01-01T00:00:00Z".into()),
            },
        ];
        assert_eq!(
            gitlab_email_from_emails(&emails),
            Some("confirmed@example.org".into())
        );
    }

    #[test]
    fn payment_links_must_be_valid_https_urls() {
        assert!(validate_payment_link("https://www.paypal.com/paypalme/Ada").is_ok());
        assert!(validate_payment_link("https://paypal.me/Ada").is_ok());
        assert!(validate_payment_link("http://paypal.me/Ada").is_err());
        assert!(validate_payment_link("not a url").is_err());
    }

    #[tokio::test]
    async fn guest_identity_reuses_existing_guest_user() {
        let state = state().await;

        let first_login = upsert_guest_user(&state, "First Guest", None, None)
            .await
            .unwrap();
        let returning_login =
            upsert_guest_user(&state, "first guest", Some(&first_login.guest_token), None)
                .await
                .unwrap();

        assert_eq!(returning_login.user_id, first_login.user_id);
        assert_eq!(returning_login.guest_token, first_login.guest_token);

        let user = load_user_by_id(&state, &first_login.user_id).await.unwrap();
        assert_eq!(user.display_name, "First Guest");
    }

    #[tokio::test]
    async fn different_guest_name_creates_new_user_even_with_browser_identity() {
        let state = state().await;

        let first_login = upsert_guest_user(&state, "test", None, None).await.unwrap();
        let second_login = upsert_guest_user(&state, "test3", Some(&first_login.guest_token), None)
            .await
            .unwrap();

        assert_ne!(second_login.user_id, first_login.user_id);
        assert_ne!(second_login.guest_token, first_login.guest_token);

        let first_user = load_user_by_id(&state, &first_login.user_id).await.unwrap();
        let second_user = load_user_by_id(&state, &second_login.user_id)
            .await
            .unwrap();
        assert_eq!(first_user.display_name, "test");
        assert_eq!(second_user.display_name, "test3");
    }

    #[tokio::test]
    async fn guest_recovery_code_reuses_guest_user_from_new_device() {
        let state = state().await;

        let first_login = upsert_guest_user(&state, "First Guest", None, None)
            .await
            .unwrap();
        let recovery_code = first_login.recovery_code.unwrap();
        let second_device_login =
            upsert_guest_user(&state, "first guest", None, Some(&recovery_code))
                .await
                .unwrap();

        assert_eq!(second_device_login.user_id, first_login.user_id);
        assert_ne!(second_device_login.guest_token, first_login.guest_token);
        assert_eq!(
            second_device_login.recovery_code.as_deref(),
            Some(recovery_code.as_str())
        );

        let user = load_user_by_id(&state, &first_login.user_id).await.unwrap();
        assert_eq!(user.display_name, "First Guest");
    }

    #[tokio::test]
    async fn recovery_code_requires_matching_guest_name() {
        let state = state().await;

        let first_login = upsert_guest_user(&state, "test", None, None).await.unwrap();
        let recovery_code = first_login.recovery_code.unwrap();
        let result = upsert_guest_user(&state, "test3", None, Some(&recovery_code)).await;

        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn provider_user_maps_existing_user_by_normalized_email() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('existing', 'member', 42, 'ada@example.org', 'Ada', 1, 0, 0, 0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "authentik-sub".into(),
                gitlab_id: None,
                email: Some(" ADA@EXAMPLE.ORG "),
                display_name: "Ada Auth",
                stock_manager: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(user_id, "existing");
        let user = load_user_by_id(&state, "existing").await.unwrap();
        assert_eq!(user.user_type, "member");
        assert_eq!(user.display_name, "Ada Auth");
        assert!(user.stock_manager);
    }

    #[tokio::test]
    async fn provider_user_creates_member_when_email_has_no_match() {
        let state = state().await;

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "new-sub".into(),
                gitlab_id: None,
                email: Some("new@example.org"),
                display_name: "New Member",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        let user = load_user_by_id(&state, &user_id).await.unwrap();
        assert_eq!(user.user_type, "member");
        assert_eq!(user.display_name, "New Member");
        assert_eq!(user.gitlab_id, None);
        assert!(!user.stock_manager);
    }

    #[tokio::test]
    async fn provider_user_does_not_map_existing_member_by_display_name() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('member-by-name', 'member', 62, 'ada@example.org', 'Ada Lovelace', 1, 0, 0, 1, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "authentik-name-sub".into(),
                gitlab_id: None,
                email: Some("ada.student@example.org"),
                display_name: "Ada Lovelace",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        assert_ne!(user_id, "member-by-name");
        let original = load_user_by_id(&state, "member-by-name").await.unwrap();
        let new_user = load_user_by_id(&state, &user_id).await.unwrap();
        assert_eq!(original.gitlab_id, Some(62));
        assert_eq!(new_user.gitlab_id, None);
        assert_eq!(original.display_name, "Ada Lovelace");
        assert_eq!(new_user.display_name, "Ada Lovelace");

        let original_email: (Option<String>,) =
            sqlx::query_as("SELECT email FROM users WHERE id = 'member-by-name'")
                .fetch_one(&state.db)
                .await
                .unwrap();
        let new_email: (Option<String>,) = sqlx::query_as("SELECT email FROM users WHERE id = ?")
            .bind(&user_id)
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(original_email.0.as_deref(), Some("ada@example.org"));
        assert_eq!(new_email.0.as_deref(), Some("ada.student@example.org"));
    }

    #[test]
    fn authentik_email_is_linkable_only_when_explicitly_verified() {
        assert_eq!(
            verified_authentik_email("ada@example.org", Some(true)),
            Some("ada@example.org")
        );
        assert_eq!(
            verified_authentik_email("ada@example.org", Some(false)),
            None
        );
        assert_eq!(verified_authentik_email("ada@example.org", None), None);
    }

    #[tokio::test]
    async fn provider_user_does_not_link_an_untrusted_email() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('verified-email-owner', 'member', NULL, 'ada@example.org', 'Ada', 1, 0, 0, 0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "unverified-email-sub".into(),
                gitlab_id: None,
                email: verified_authentik_email("ada@example.org", None),
                display_name: "Ada",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        assert_ne!(user_id, "verified-email-owner");
        let new_email: (Option<String>,) = sqlx::query_as("SELECT email FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(new_email.0, None);
    }

    #[tokio::test]
    async fn existing_provider_subject_maps_without_a_trusted_email() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('known-subject-user', 'member', NULL, 'old@example.org', 'Old Name', 1, 0, 0, 0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();
        upsert_auth_identity(
            &state,
            "authentik",
            "known-authentik-sub",
            "known-subject-user",
            now,
        )
        .await
        .unwrap();

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "known-authentik-sub".into(),
                gitlab_id: None,
                email: verified_authentik_email("new@example.org", None),
                display_name: "New Name",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(user_id, "known-subject-user");
        let stored: (Option<String>, String) =
            sqlx::query_as("SELECT email, display_name FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(stored.0.as_deref(), Some("old@example.org"));
        assert_eq!(stored.1, "New Name");
    }

    #[tokio::test]
    async fn linked_authentik_staff_login_keeps_stock_manager_permission() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('linked-staff', 'member', 62, 'staff@example.org', 'Staff User', 1, 0, 1, 1, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();

        let groups = vec!["coffee-admins".to_string()];
        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "authentik-linked-staff-sub".into(),
                gitlab_id: None,
                email: Some("staff@example.org"),
                display_name: "Staff User",
                stock_manager: string_has_stock_manager_group(
                    &groups,
                    &state.config.auth_staff_groups,
                ),
            },
        )
        .await
        .unwrap();

        assert_eq!(user_id, "linked-staff");
        let user = load_user_by_id(&state, "linked-staff").await.unwrap();
        assert!(user.stock_manager);
    }

    #[tokio::test]
    async fn provider_user_does_not_map_guest_by_display_name() {
        let state = state().await;
        let now = now_ts();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES ('guest-by-name', 'guest', NULL, NULL, 'Ada Lovelace', 0, 0, 0, 0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .unwrap();

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "authentik-new-member-sub".into(),
                gitlab_id: None,
                email: None,
                display_name: "Ada Lovelace",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        assert_ne!(user_id, "guest-by-name");
        let user = load_user_by_id(&state, &user_id).await.unwrap();
        assert_eq!(user.user_type, "member");
        assert_eq!(user.display_name, "Ada Lovelace");
    }

    #[tokio::test]
    async fn provider_user_does_not_map_ambiguous_member_display_name() {
        let state = state().await;
        let now = now_ts();
        for id in ["member-one", "member-two"] {
            sqlx::query(
                "INSERT INTO users
                 (id, user_type, gitlab_id, email, display_name, leaderboard_opt_in,
                  achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
                 VALUES (?, 'member', NULL, NULL, 'Shared Name', 1, 0, 0, 0, ?, ?)",
            )
            .bind(id)
            .bind(now)
            .bind(now)
            .execute(&state.db)
            .await
            .unwrap();
        }

        let user_id = upsert_provider_user(
            &state,
            ProviderUser {
                provider: "authentik",
                subject: "authentik-ambiguous-name-sub".into(),
                gitlab_id: None,
                email: None,
                display_name: "Shared Name",
                stock_manager: false,
            },
        )
        .await
        .unwrap();

        assert_ne!(user_id, "member-one");
        assert_ne!(user_id, "member-two");
        let user = load_user_by_id(&state, &user_id).await.unwrap();
        assert_eq!(user.user_type, "member");
        assert_eq!(user.display_name, "Shared Name");
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-auth-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        AppState {
            config: Config {
                app_name: "Coffee Tracker".into(),
                database_url: format!("sqlite://{db_path}"),
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                session_secret: "test".into(),
                gitlab_base_url: "https://gitlab.example.org".into(),
                gitlab_oauth_client_id: String::new(),
                gitlab_oauth_client_secret: String::new(),
                gitlab_oauth_scopes: "read_user".into(),
                authentik_issuer_url: String::new(),
                authentik_oauth_client_id: String::new(),
                authentik_oauth_client_secret: String::new(),
                authentik_oauth_scopes: "openid email profile groups".into(),
                auth_allowed_groups: Vec::new(),
                auth_staff_groups: vec!["coffee-admins".into()],
                public_base_url: "http://localhost".into(),
                frontend_base_url: "http://localhost".into(),
                mattermost_webhook_url: None,
                matrix_webhook_url: None,
                smtp: None,
                paypal_link: None,
                auto_payment_reminder: None,
                timezone: "UTC".into(),
                session_ttl: std::time::Duration::from_secs(60),
            },
            db,
            http: reqwest::Client::new(),
        }
    }
}
