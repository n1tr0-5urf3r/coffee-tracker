use std::{env, net::SocketAddr, time::Duration};

use anyhow::{Context, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmtpSecurity {
    None,
    Starttls,
}

#[derive(Clone, Debug)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub from: String,
    pub security: SmtpSecurity,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub app_name: String,
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub session_secret: String,
    pub gitlab_base_url: String,
    pub gitlab_oauth_client_id: String,
    pub gitlab_oauth_client_secret: String,
    pub gitlab_oauth_scopes: String,
    pub authentik_issuer_url: String,
    pub authentik_oauth_client_id: String,
    pub authentik_oauth_client_secret: String,
    pub authentik_oauth_scopes: String,
    pub auth_allowed_groups: Vec<String>,
    pub auth_staff_groups: Vec<String>,
    pub public_base_url: String,
    pub frontend_base_url: String,
    pub mattermost_webhook_url: Option<String>,
    pub matrix_webhook_url: Option<String>,
    pub smtp: Option<SmtpConfig>,
    pub paypal_link: Option<String>,
    pub timezone: String,
    pub session_ttl: Duration,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let app_name = env::var("APP_NAME").unwrap_or_else(|_| "Coffee Tracker".into());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://coffee-tracker.db".to_string());
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:3124".to_string())
            .parse()?;
        let session_secret =
            env::var("SESSION_SECRET").unwrap_or_else(|_| "dev-session-secret-change-me".into());
        let gitlab_base_url =
            env::var("GITLAB_BASE_URL").unwrap_or_else(|_| "https://gitlab.example.org".into());
        let gitlab_oauth_client_id = env::var("GITLAB_OAUTH_CLIENT_ID").unwrap_or_default();
        let gitlab_oauth_client_secret = env::var("GITLAB_OAUTH_CLIENT_SECRET").unwrap_or_default();
        let gitlab_oauth_scopes =
            env::var("GITLAB_OAUTH_SCOPES").unwrap_or_else(|_| "read_user read_api".into());
        let authentik_issuer_url = env::var("AUTHENTIK_ISSUER_URL").unwrap_or_default();
        let authentik_oauth_client_id = env::var("AUTHENTIK_OAUTH_CLIENT_ID").unwrap_or_default();
        let authentik_oauth_client_secret =
            env::var("AUTHENTIK_OAUTH_CLIENT_SECRET").unwrap_or_default();
        let authentik_oauth_scopes = env::var("AUTHENTIK_OAUTH_SCOPES")
            .unwrap_or_else(|_| "openid email profile groups".into());
        let auth_allowed_groups = parse_csv_env(
            env::var("AUTH_ALLOWED_GROUPS").unwrap_or_else(|_| "coffee-users,coffee-admins".into()),
        );
        let auth_staff_groups =
            parse_csv_env(env::var("AUTH_STAFF_GROUPS").unwrap_or_else(|_| "coffee-admins".into()));
        let public_base_url =
            env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:3124".into());
        let frontend_base_url =
            env::var("FRONTEND_BASE_URL").unwrap_or_else(|_| "http://localhost:3123".into());
        let mattermost_webhook_url = optional_env("MATTERMOST_WEBHOOK_URL");
        let matrix_webhook_url = optional_env("MATRIX_WEBHOOK_URL");
        let smtp = smtp_config_from_env()?;
        let paypal_link = env::var("PAYPAL_LINK").ok().and_then(non_empty_trimmed);
        let timezone = env::var("TIMEZONE").unwrap_or_else(|_| "Europe/Berlin".into());

        Ok(Self {
            app_name,
            database_url,
            bind_addr,
            session_secret,
            gitlab_base_url: gitlab_base_url.trim_end_matches('/').to_string(),
            gitlab_oauth_client_id,
            gitlab_oauth_client_secret,
            gitlab_oauth_scopes,
            authentik_issuer_url: authentik_issuer_url.trim_end_matches('/').to_string(),
            authentik_oauth_client_id,
            authentik_oauth_client_secret,
            authentik_oauth_scopes,
            auth_allowed_groups,
            auth_staff_groups,
            public_base_url: public_base_url.trim_end_matches('/').to_string(),
            frontend_base_url: frontend_base_url.trim_end_matches('/').to_string(),
            mattermost_webhook_url,
            matrix_webhook_url,
            smtp,
            paypal_link,
            timezone,
            session_ttl: Duration::from_secs(60 * 60 * 24 * 180),
        })
    }
}

fn smtp_config_from_env() -> anyhow::Result<Option<SmtpConfig>> {
    build_smtp_config(
        optional_env("SMTP_HOST"),
        optional_env("SMTP_FROM"),
        optional_env("SMTP_PORT"),
        optional_env("SMTP_SECURITY"),
    )
}

fn build_smtp_config(
    host: Option<String>,
    from: Option<String>,
    port: Option<String>,
    security: Option<String>,
) -> anyhow::Result<Option<SmtpConfig>> {
    let (host, from) = match (host, from) {
        (None, None) => return Ok(None),
        (Some(host), Some(from)) => (host, from),
        _ => bail!("SMTP_HOST and SMTP_FROM must either both be set or both be blank"),
    };

    from.parse::<lettre::message::Mailbox>()
        .with_context(|| "SMTP_FROM must be a valid mailbox")?;

    let port = port
        .unwrap_or_else(|| "25".into())
        .parse::<u16>()
        .with_context(|| "SMTP_PORT must be a valid TCP port")?;
    let security = match security
        .unwrap_or_else(|| "none".into())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "none" => SmtpSecurity::None,
        "starttls" => SmtpSecurity::Starttls,
        _ => bail!("SMTP_SECURITY must be either 'none' or 'starttls'"),
    };

    Ok(Some(SmtpConfig {
        host,
        port,
        from,
        security,
    }))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(non_empty_trimmed)
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parse_csv_env(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SmtpSecurity, build_smtp_config, non_empty_trimmed, parse_csv_env};

    #[test]
    fn blank_optional_env_values_are_disabled() {
        assert_eq!(non_empty_trimmed(String::new()), None);
        assert_eq!(non_empty_trimmed("   ".to_string()), None);
        assert_eq!(
            non_empty_trimmed(" https://matrix.example/hook ".to_string()),
            Some("https://matrix.example/hook".to_string())
        );
    }

    #[test]
    fn auth_allowed_groups_are_parsed_from_csv() {
        assert_eq!(
            parse_csv_env(" coffee-admins,coffee-users,, guests ".to_string()),
            vec!["coffee-admins", "coffee-users", "guests"]
        );
    }

    #[test]
    fn smtp_config_supports_plain_and_starttls_relays() {
        let plain = build_smtp_config(
            Some("postfix.internal".into()),
            Some("Coffee Tracker <coffee@example.org>".into()),
            None,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plain.port, 25);
        assert_eq!(plain.security, SmtpSecurity::None);

        let starttls = build_smtp_config(
            Some("smtp.example.org".into()),
            Some("coffee@example.org".into()),
            Some("587".into()),
            Some("STARTTLS".into()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(starttls.port, 587);
        assert_eq!(starttls.security, SmtpSecurity::Starttls);
    }

    #[test]
    fn smtp_config_rejects_incomplete_or_invalid_values() {
        assert!(build_smtp_config(Some("smtp".into()), None, None, None).is_err());
        assert!(
            build_smtp_config(Some("smtp".into()), Some("invalid".into()), None, None,).is_err()
        );
        assert!(
            build_smtp_config(
                Some("smtp".into()),
                Some("coffee@example.org".into()),
                Some("invalid".into()),
                None,
            )
            .is_err()
        );
        assert!(
            build_smtp_config(
                Some("smtp".into()),
                Some("coffee@example.org".into()),
                None,
                Some("smtps".into()),
            )
            .is_err()
        );
    }
}
