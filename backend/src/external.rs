//! Read-only API for trusted services such as a game server.
//!
//! Disabled (404) unless `EXTERNAL_API_TOKEN` is set. Every request needs
//! `Authorization: Bearer <token>` and, when `EXTERNAL_API_ALLOWED_IPS` is set,
//! must come from an allowed address. Only member accounts that keep
//! achievement sharing on are exposed; guests and opted-out users look like
//! they don't exist. No payments, emails or other private data leave here.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, FromRequestParts, Path, Query, State},
    http::{HeaderMap, HeaderValue, header, request::Parts},
    middleware,
    response::Response,
    routing::get,
};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};

use crate::{
    AppState,
    config::IpCidr,
    cosmetics::{
        self, EquippedCosmetics,
        catalog::{self, Rarity, Slot},
    },
    error::{ApiError, ApiResult},
    stats::resolve_tz,
};

const DEFAULT_DAYS: u32 = 30;
const MAX_DAYS: u32 = 366;
const DEFAULT_DRINKS_LIMIT: u32 = 100;
const MAX_DRINKS_LIMIT: u32 = 1000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/external/users", get(list_users))
        .route("/external/users/{user_id}", get(user_profile))
        .route("/external/users/{user_id}/drinks", get(user_drinks))
        .layer(middleware::map_response(no_store))
}

/// Responses carry personal data, so no proxy or client may cache them.
async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A caller that passed the IP allowlist and presented a valid service token.
pub struct ExternalClient;

impl FromRequestParts<AppState> for ExternalClient {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = authorize(parts, state);
        async move { result }
    }
}

fn authorize(parts: &Parts, state: &AppState) -> ApiResult<ExternalClient> {
    let Some(config) = state.config.external_api.as_ref() else {
        return Err(ApiError::NotFound);
    };
    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip());
    let client_ip = client_ip(peer, &parts.headers, &config.trusted_proxies);

    if !config.allowed_ips.is_empty()
        && !client_ip.is_some_and(|ip| config.allowed_ips.iter().any(|net| net.contains(ip)))
    {
        tracing::warn!(?client_ip, ?peer, path = %parts.uri.path(), "external API: address not allowed");
        return Err(ApiError::Forbidden);
    }

    let valid = bearer_token(&parts.headers)
        .is_some_and(|token| token_matches(token, &config.token_digests));
    if !valid {
        tracing::warn!(?client_ip, path = %parts.uri.path(), "external API: missing or invalid token");
        return Err(ApiError::Unauthorized);
    }
    Ok(ExternalClient)
}

/// The address the request came from. `X-Forwarded-For` is only read when the
/// direct peer is a trusted proxy, walking from the right and skipping further
/// trusted proxies, so a client can't spoof its address by sending the header
/// itself. Returns `None` (deny) when the address can't be determined.
fn client_ip(peer: Option<IpAddr>, headers: &HeaderMap, trusted: &[IpCidr]) -> Option<IpAddr> {
    let is_trusted = |ip: IpAddr| trusted.iter().any(|net| net.contains(ip));
    let mut client = peer?.to_canonical();
    if !is_trusted(client) {
        return Some(client);
    }
    let mut hops = Vec::new();
    for value in headers.get_all("x-forwarded-for") {
        hops.extend(value.to_str().ok()?.split(',').map(str::trim));
    }
    for hop in hops.into_iter().rev() {
        client = hop.parse::<IpAddr>().ok()?.to_canonical();
        if !is_trusted(client) {
            break;
        }
    }
    Some(client)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// Compares SHA-256 digests in constant time and checks every configured
/// token, so timing reveals neither the token nor which one matched.
fn token_matches(token: &str, digests: &[[u8; 32]]) -> bool {
    let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    digests.iter().fold(false, |found, expected| {
        let diff = digest
            .iter()
            .zip(expected)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        found | (diff == 0)
    })
}

#[derive(Serialize, ToSchema)]
pub struct ExternalIdentity {
    /// Login provider, e.g. `gitlab` or `authentik`.
    pub provider: String,
    /// The provider's stable user ID (GitLab user ID, OIDC `sub`).
    pub subject: String,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalUser {
    pub id: String,
    pub display_name: String,
    pub identities: Vec<ExternalIdentity>,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalUsersResponse {
    pub users: Vec<ExternalUser>,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalAchievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub unlocked_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalCosmetic {
    pub id: String,
    pub slot: Slot,
    pub rarity: Rarity,
    pub name: String,
    pub acquired_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DailyDrinks {
    /// Local date (`YYYY-MM-DD`) in `timezone`.
    pub date: String,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalDrinkSummary {
    /// Every drink ever logged.
    pub total: i64,
    /// Drinks logged today (local time).
    pub today: i64,
    /// Timezone the dates are in.
    pub timezone: String,
    /// One entry per day, oldest first, ending today; days without drinks count 0.
    pub daily: Vec<DailyDrinks>,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalUserProfile {
    pub user: ExternalUser,
    /// Unlocked achievements, oldest unlock first.
    pub achievements: Vec<ExternalAchievement>,
    /// Owned cosmetics, oldest first.
    pub cosmetics: Vec<ExternalCosmetic>,
    pub equipped: EquippedCosmetics,
    pub drinks: ExternalDrinkSummary,
    /// Unix seconds when this snapshot was taken.
    pub generated_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalDrink {
    pub id: String,
    pub beverage_id: String,
    pub beverage_name: String,
    pub machine_id: String,
    /// Unix seconds.
    pub consumed_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalDrinksResponse {
    /// Newest first.
    pub drinks: Vec<ExternalDrink>,
    /// More drinks match; fetch them with `until` set to the oldest `consumed_at` returned.
    pub has_more: bool,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListUsersQuery {
    /// Only the user with this email address (case-insensitive).
    pub email: Option<String>,
    /// Only the user linked to this login provider (`gitlab`, `authentik`); needs `subject`.
    pub provider: Option<String>,
    /// The provider's user ID; needs `provider`.
    pub subject: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProfileQuery {
    /// Number of days in `drinks.daily`, including today (1–366, default 30).
    pub days: Option<u32>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DrinksQuery {
    /// Only drinks at or after this Unix timestamp.
    pub since: Option<i64>,
    /// Only drinks before this Unix timestamp (exclusive).
    pub until: Option<i64>,
    /// Maximum number of drinks (1–1000, default 100).
    pub limit: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/external/users",
    tag = "external",
    params(ListUsersQuery),
    security(("external_token" = [])),
    responses(
        (status = 200, body = ExternalUsersResponse),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Caller address not allowed"),
        (status = 404, description = "External API disabled")
    )
)]
pub(crate) async fn list_users(
    State(state): State<AppState>,
    _client: ExternalClient,
    Query(query): Query<ListUsersQuery>,
) -> ApiResult<Json<ExternalUsersResponse>> {
    let email = query
        .email
        .map(|email| email.trim().to_ascii_lowercase())
        .filter(|email| !email.is_empty());
    let identity = match (query.provider, query.subject) {
        (Some(provider), Some(subject)) => Some((provider, subject)),
        (None, None) => None,
        _ => {
            return Err(ApiError::BadRequest(
                "provider and subject must be given together".into(),
            ));
        }
    };
    let (provider, subject) = identity.unzip();

    let rows: Vec<(String, String)> = sqlx::query_as(
        // Only members who keep achievement sharing on are ever exposed.
        "SELECT u.id, u.display_name FROM users u
         WHERE u.user_type = 'member' AND u.achievement_sharing_opt_out = 0
           AND (?1 IS NULL OR lower(u.email) = ?1)
           AND (?2 IS NULL OR EXISTS (
                 SELECT 1 FROM auth_identities ai
                 WHERE ai.user_id = u.id AND ai.provider = ?2 AND ai.subject = ?3))
         ORDER BY u.display_name COLLATE NOCASE, u.id",
    )
    .bind(email)
    .bind(provider)
    .bind(subject)
    .fetch_all(&state.db)
    .await?;

    let mut identities: HashMap<String, Vec<ExternalIdentity>> = HashMap::new();
    let identity_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT ai.user_id, ai.provider, ai.subject
         FROM auth_identities ai JOIN users u ON u.id = ai.user_id
         WHERE u.user_type = 'member' AND u.achievement_sharing_opt_out = 0
         ORDER BY ai.provider, ai.subject",
    )
    .fetch_all(&state.db)
    .await?;
    for (user_id, provider, subject) in identity_rows {
        identities
            .entry(user_id)
            .or_default()
            .push(ExternalIdentity { provider, subject });
    }

    let users = rows
        .into_iter()
        .map(|(id, display_name)| ExternalUser {
            identities: identities.remove(&id).unwrap_or_default(),
            id,
            display_name,
        })
        .collect();
    Ok(Json(ExternalUsersResponse { users }))
}

#[utoipa::path(
    get,
    path = "/api/v1/external/users/{user_id}",
    tag = "external",
    params(("user_id" = String, Path, description = "Coffee Tracker user ID"), ProfileQuery),
    security(("external_token" = [])),
    responses(
        (status = 200, body = ExternalUserProfile),
        (status = 400, description = "Invalid `days`"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Caller address not allowed"),
        (status = 404, description = "Unknown user, guest, opted out, or external API disabled")
    )
)]
pub(crate) async fn user_profile(
    State(state): State<AppState>,
    _client: ExternalClient,
    Path(user_id): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> ApiResult<Json<ExternalUserProfile>> {
    let days = query.days.unwrap_or(DEFAULT_DAYS);
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(ApiError::BadRequest(format!(
            "days must be between 1 and {MAX_DAYS}"
        )));
    }
    let user = load_user(&state, &user_id).await?;
    let now = Utc::now().timestamp();

    let achievements = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT d.id, d.name, d.description, u.unlocked_at
         FROM achievement_unlocks u
         JOIN achievement_definitions d ON d.id = u.achievement_id
         WHERE u.user_id = ?
         ORDER BY u.unlocked_at, d.id",
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, name, description, unlocked_at)| ExternalAchievement {
        id,
        name,
        description,
        unlocked_at,
    })
    .collect();

    let cosmetics = sqlx::query_as::<_, (String, i64)>(
        "SELECT item_id, acquired_at FROM user_cosmetics
         WHERE user_id = ? ORDER BY acquired_at, item_id",
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .filter_map(|(item_id, acquired_at)| {
        catalog::find(&item_id).map(|item| ExternalCosmetic {
            id: item.id.into(),
            slot: item.slot,
            rarity: item.rarity,
            name: item.name.into(),
            acquired_at,
        })
    })
    .collect();
    let (equipped, _) = cosmetics::load_equipped(&state, &user.id).await?;

    let drinks = drink_summary(&state, &user.id, days, now).await?;

    Ok(Json(ExternalUserProfile {
        user,
        achievements,
        cosmetics,
        equipped,
        drinks,
        generated_at: now,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/external/users/{user_id}/drinks",
    tag = "external",
    params(("user_id" = String, Path, description = "Coffee Tracker user ID"), DrinksQuery),
    security(("external_token" = [])),
    responses(
        (status = 200, body = ExternalDrinksResponse),
        (status = 400, description = "Invalid `limit`"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Caller address not allowed"),
        (status = 404, description = "Unknown user, guest, opted out, or external API disabled")
    )
)]
pub(crate) async fn user_drinks(
    State(state): State<AppState>,
    _client: ExternalClient,
    Path(user_id): Path<String>,
    Query(query): Query<DrinksQuery>,
) -> ApiResult<Json<ExternalDrinksResponse>> {
    let limit = query.limit.unwrap_or(DEFAULT_DRINKS_LIMIT);
    if !(1..=MAX_DRINKS_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "limit must be between 1 and {MAX_DRINKS_LIMIT}"
        )));
    }
    let user = load_user(&state, &user_id).await?;

    let mut drinks = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT c.id, c.beverage_id, b.name, c.machine_id, c.consumed_at
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ?
           AND (? IS NULL OR c.consumed_at >= ?)
           AND (? IS NULL OR c.consumed_at < ?)
         ORDER BY c.consumed_at DESC, c.id DESC
         LIMIT ?",
    )
    .bind(&user.id)
    .bind(query.since)
    .bind(query.since)
    .bind(query.until)
    .bind(query.until)
    .bind(i64::from(limit) + 1)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(
        |(id, beverage_id, beverage_name, machine_id, consumed_at)| ExternalDrink {
            id,
            beverage_id,
            beverage_name,
            machine_id,
            consumed_at,
        },
    )
    .collect::<Vec<_>>();
    let has_more = drinks.len() > limit as usize;
    drinks.truncate(limit as usize);
    Ok(Json(ExternalDrinksResponse { drinks, has_more }))
}

async fn load_user(state: &AppState, user_id: &str) -> ApiResult<ExternalUser> {
    let (id, display_name): (String, String) = sqlx::query_as(
        "SELECT u.id, u.display_name FROM users u
         WHERE u.id = ? AND u.user_type = 'member' AND u.achievement_sharing_opt_out = 0",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;
    let identities = sqlx::query_as::<_, (String, String)>(
        "SELECT provider, subject FROM auth_identities WHERE user_id = ? ORDER BY provider, subject",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(provider, subject)| ExternalIdentity { provider, subject })
    .collect();
    Ok(ExternalUser {
        id,
        display_name,
        identities,
    })
}

async fn drink_summary(
    state: &AppState,
    user_id: &str,
    days: u32,
    now: i64,
) -> ApiResult<ExternalDrinkSummary> {
    let tz = resolve_tz(&state.config.timezone);
    let local_date = |timestamp: i64| -> Option<NaiveDate> {
        Utc.timestamp_opt(timestamp, 0)
            .single()
            .map(|dt| dt.with_timezone(&tz).date_naive())
    };
    let today = local_date(now).ok_or_else(|| anyhow::anyhow!("invalid current time"))?;
    let first_day = today - Duration::days(i64::from(days) - 1);

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM consumptions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    // One spare day on each side covers any UTC offset; the local date decides.
    let window_start = now - (i64::from(days) + 1) * 86_400;
    let timestamps: Vec<(i64,)> = sqlx::query_as(
        "SELECT consumed_at FROM consumptions WHERE user_id = ? AND consumed_at >= ?",
    )
    .bind(user_id)
    .bind(window_start)
    .fetch_all(&state.db)
    .await?;
    let mut counts: HashMap<NaiveDate, i64> = HashMap::new();
    for (timestamp,) in timestamps {
        if let Some(date) = local_date(timestamp).filter(|d| *d >= first_day && *d <= today) {
            *counts.entry(date).or_default() += 1;
        }
    }

    let daily = first_day
        .iter_days()
        .take_while(|date| *date <= today)
        .map(|date| DailyDrinks {
            date: date.format("%Y-%m-%d").to_string(),
            count: counts.get(&date).copied().unwrap_or(0),
        })
        .collect();

    Ok(ExternalDrinkSummary {
        total,
        today: counts.get(&today).copied().unwrap_or(0),
        timezone: tz.name().to_string(),
        daily,
    })
}

#[cfg(test)]
mod tests {
    use axum::{
        Extension,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;
    use crate::{config::ExternalApiConfig, cosmetics::test_support::state_with_users};

    const TOKEN: &str = "test-token-0123456789abcdef0123456789";

    fn cidrs(values: &[&str]) -> Vec<IpCidr> {
        values.iter().map(|v| IpCidr::parse(v).unwrap()).collect()
    }

    fn external_config(allowed: &[&str], proxies: &[&str]) -> ExternalApiConfig {
        ExternalApiConfig {
            token_digests: vec![Sha256::digest(TOKEN.as_bytes()).into()],
            allowed_ips: cidrs(allowed),
            trusted_proxies: cidrs(proxies),
        }
    }

    async fn app(state: &AppState, peer: &str) -> Router {
        crate::build_app(state.config.clone(), state.db.clone())
            .await
            .unwrap()
            .layer(Extension(ConnectInfo(SocketAddr::new(
                peer.parse().unwrap(),
                40000,
            ))))
    }

    async fn get(
        app: &Router,
        uri: &str,
        token: Option<&str>,
        forwarded_for: Option<&str>,
    ) -> (StatusCode, HeaderMap, serde_json::Value) {
        let mut request = Request::get(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(forwarded_for) = forwarded_for {
            request = request.header("x-forwarded-for", forwarded_for);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, headers, body)
    }

    async fn add_drink(state: &AppState, user_id: &str, beverage_id: &str, at: i64) {
        sqlx::query(
            "INSERT INTO consumptions (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES (?, ?, 'main', ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(beverage_id)
        .bind(at)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[test]
    fn forwarded_for_is_only_believed_from_trusted_proxies() {
        let proxies = cidrs(&["172.16.0.0/12"]);
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "6.6.6.6, 10.0.0.5".parse().unwrap());

        // A direct caller can't spoof its address with the header.
        assert_eq!(
            client_ip(Some("8.8.8.8".parse().unwrap()), &headers, &proxies),
            Some("8.8.8.8".parse().unwrap())
        );
        // Through a trusted proxy, the rightmost untrusted hop is the client.
        assert_eq!(
            client_ip(Some("172.18.0.2".parse().unwrap()), &headers, &proxies),
            Some("10.0.0.5".parse().unwrap())
        );
        // Chained trusted proxies are skipped.
        headers.insert(
            "x-forwarded-for",
            "6.6.6.6, 10.0.0.5, 172.18.0.3".parse().unwrap(),
        );
        assert_eq!(
            client_ip(Some("172.18.0.2".parse().unwrap()), &headers, &proxies),
            Some("10.0.0.5".parse().unwrap())
        );
        // Garbage in the chain fails closed.
        headers.insert("x-forwarded-for", "10.0.0.5, nonsense".parse().unwrap());
        assert_eq!(
            client_ip(Some("172.18.0.2".parse().unwrap()), &headers, &proxies),
            None
        );
        // Without a known peer there is no address.
        assert_eq!(client_ip(None, &HeaderMap::new(), &proxies), None);
    }

    #[test]
    fn bearer_token_and_digest_matching() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        headers.insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);
        headers.insert(header::AUTHORIZATION, "bearer abc".parse().unwrap());
        assert_eq!(bearer_token(&headers), Some("abc"));

        let digests = vec![
            Sha256::digest(b"first-token").into(),
            Sha256::digest(b"second-token").into(),
        ];
        assert!(token_matches("first-token", &digests));
        assert!(token_matches("second-token", &digests));
        assert!(!token_matches("first-toke", &digests));
        assert!(!token_matches("", &digests));
    }

    #[tokio::test]
    async fn disabled_api_answers_not_found() {
        let state = state_with_users(&["alice"]).await;
        let app = app(&state, "10.0.0.5").await;
        let (status, _, _) = get(&app, "/api/v1/external/users", Some(TOKEN), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn requires_a_valid_token_and_an_allowed_address() {
        let mut state = state_with_users(&["alice"]).await;
        state.config.external_api = Some(external_config(&["10.0.0.5"], &["172.16.0.0/12"]));

        let allowed = app(&state, "10.0.0.5").await;
        let (status, headers, _) = get(&allowed, "/api/v1/external/users", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        let (status, _, _) = get(&allowed, "/api/v1/external/users", Some("wrong"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, headers, body) =
            get(&allowed, "/api/v1/external/users", Some(TOKEN), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(body["users"][0]["id"], "alice");

        // The right token from the wrong address is still refused.
        let other = app(&state, "10.0.0.6").await;
        let (status, _, _) = get(&other, "/api/v1/external/users", Some(TOKEN), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // ...even when it claims to be the allowed address.
        let (status, _, _) = get(
            &other,
            "/api/v1/external/users",
            Some(TOKEN),
            Some("10.0.0.5"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Behind a trusted proxy, the forwarded address counts.
        let proxied = app(&state, "172.18.0.2").await;
        let (status, _, _) = get(
            &proxied,
            "/api/v1/external/users",
            Some(TOKEN),
            Some("10.0.0.5"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = get(
            &proxied,
            "/api/v1/external/users",
            Some(TOKEN),
            Some("10.0.0.6"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn only_exposes_members_who_share_achievements() {
        let mut state = state_with_users(&["alice", "bob", "carol"]).await;
        state.config.external_api = Some(external_config(&[], &[]));
        sqlx::query("UPDATE users SET achievement_sharing_opt_out = 1 WHERE id = 'bob'")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET user_type = 'guest' WHERE id = 'carol'")
            .execute(&state.db)
            .await
            .unwrap();
        let app = app(&state, "10.0.0.5").await;

        let (_, _, body) = get(&app, "/api/v1/external/users", Some(TOKEN), None).await;
        let ids: Vec<_> = body["users"]
            .as_array()
            .unwrap()
            .iter()
            .map(|user| user["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, vec!["alice"]);

        for id in ["bob", "carol", "nobody"] {
            let (status, _, _) = get(
                &app,
                &format!("/api/v1/external/users/{id}"),
                Some(TOKEN),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
            let (status, _, _) = get(
                &app,
                &format!("/api/v1/external/users/{id}/drinks"),
                Some(TOKEN),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
        }
    }

    #[tokio::test]
    async fn users_can_be_looked_up_by_email_or_login_identity() {
        let mut state = state_with_users(&["alice", "bob"]).await;
        state.config.external_api = Some(external_config(&[], &[]));
        sqlx::query("UPDATE users SET email = 'alice@example.org' WHERE id = 'alice'")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO auth_identities (provider, subject, user_id, created_at, updated_at)
             VALUES ('gitlab', '42', 'bob', 1, 1)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        let app = app(&state, "10.0.0.5").await;

        let (_, _, body) = get(
            &app,
            "/api/v1/external/users?email=Alice@Example.org",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(body["users"].as_array().unwrap().len(), 1);
        assert_eq!(body["users"][0]["id"], "alice");
        assert!(body["users"][0].get("email").is_none());

        let (_, _, body) = get(
            &app,
            "/api/v1/external/users?provider=gitlab&subject=42",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(body["users"].as_array().unwrap().len(), 1);
        assert_eq!(body["users"][0]["id"], "bob");
        assert_eq!(body["users"][0]["identities"][0]["subject"], "42");

        let (status, _, _) = get(
            &app,
            "/api/v1/external/users?provider=gitlab",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn profile_lists_achievements_cosmetics_and_daily_drinks() {
        let mut state = state_with_users(&["alice"]).await;
        state.config.external_api = Some(external_config(&[], &[]));
        let (achievement_id,): (String,) =
            sqlx::query_as("SELECT id FROM achievement_definitions LIMIT 1")
                .fetch_one(&state.db)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO achievement_unlocks (id, user_id, achievement_id, unlocked_at)
             VALUES ('u1', 'alice', ?, 100)",
        )
        .bind(&achievement_id)
        .execute(&state.db)
        .await
        .unwrap();
        let avatar = catalog::CATALOG
            .iter()
            .find(|item| item.slot == Slot::Avatar)
            .unwrap();
        sqlx::query(
            "INSERT INTO user_cosmetics (user_id, item_id, source, acquired_at)
             VALUES ('alice', ?, 'lootbox', 200)",
        )
        .bind(avatar.id)
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO user_equipped_cosmetics (user_id, slot, item_id) VALUES ('alice', 'avatar', ?)",
        )
        .bind(avatar.id)
        .execute(&state.db)
        .await
        .unwrap();
        let now = Utc::now().timestamp();
        add_drink(&state, "alice", "coffee", now - 10).await;
        add_drink(&state, "alice", "coffee", now - 20).await;
        add_drink(&state, "alice", "coffee", now - 400 * 86_400).await;
        let app = app(&state, "10.0.0.5").await;

        let (status, _, body) = get(
            &app,
            "/api/v1/external/users/alice?days=7",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"]["display_name"], "ALICE");
        assert_eq!(body["achievements"][0]["id"], achievement_id.as_str());
        assert_eq!(body["achievements"][0]["unlocked_at"], 100);
        assert_eq!(body["cosmetics"][0]["id"], avatar.id);
        assert_eq!(body["cosmetics"][0]["slot"], "avatar");
        assert_eq!(body["equipped"]["avatar"], avatar.id);
        assert_eq!(body["drinks"]["total"], 3);
        assert_eq!(body["drinks"]["today"], 2);
        assert_eq!(body["drinks"]["timezone"], "Europe/Berlin");
        let daily = body["drinks"]["daily"].as_array().unwrap();
        assert_eq!(daily.len(), 7);
        assert_eq!(daily[6]["count"], 2);
        assert_eq!(
            daily
                .iter()
                .map(|d| d["count"].as_i64().unwrap())
                .sum::<i64>(),
            2
        );

        for days in ["0", "367"] {
            let (status, _, _) = get(
                &app,
                &format!("/api/v1/external/users/alice?days={days}"),
                Some(TOKEN),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn drinks_are_listed_newest_first_and_paged() {
        let mut state = state_with_users(&["alice", "bob"]).await;
        state.config.external_api = Some(external_config(&[], &[]));
        for at in [1_000, 2_000, 3_000] {
            add_drink(&state, "alice", "coffee", at).await;
        }
        add_drink(&state, "bob", "coffee", 2_500).await;
        let app = app(&state, "10.0.0.5").await;

        let (status, _, body) = get(
            &app,
            "/api/v1/external/users/alice/drinks?limit=2",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let times: Vec<_> = body["drinks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["consumed_at"].as_i64().unwrap())
            .collect();
        assert_eq!(times, vec![3_000, 2_000]);
        assert_eq!(body["has_more"], true);
        assert_eq!(body["drinks"][0]["beverage_id"], "coffee");
        assert!(body["drinks"][0]["beverage_name"].is_string());

        let (_, _, body) = get(
            &app,
            "/api/v1/external/users/alice/drinks?limit=2&until=2000",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(body["drinks"].as_array().unwrap().len(), 1);
        assert_eq!(body["drinks"][0]["consumed_at"], 1_000);
        assert_eq!(body["has_more"], false);

        let (_, _, body) = get(
            &app,
            "/api/v1/external/users/alice/drinks?since=1500&until=2500",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(body["drinks"].as_array().unwrap().len(), 1);
        assert_eq!(body["drinks"][0]["consumed_at"], 2_000);

        let (status, _, _) = get(
            &app,
            "/api/v1/external/users/alice/drinks?limit=0",
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
