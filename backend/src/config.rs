use std::{
    env,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoPaymentReminderConfig {
    pub payment_link_user_id: String,
    pub hour: u32,
}

/// Minimum length of an `EXTERNAL_API_TOKEN` entry, so tokens can't be guessed.
pub const EXTERNAL_API_TOKEN_MIN_LEN: usize = 32;

/// Read-only API for trusted services such as a game server (`/api/v1/external/*`).
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalApiConfig {
    /// SHA-256 digests of the accepted bearer tokens; the tokens themselves are
    /// never kept in memory after startup.
    pub token_digests: Vec<[u8; 32]>,
    /// Client addresses allowed to call the API; empty means any address.
    pub allowed_ips: Vec<IpCidr>,
    /// Reverse proxies whose `X-Forwarded-For` header is believed.
    pub trusted_proxies: Vec<IpCidr>,
}

impl std::fmt::Debug for ExternalApiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalApiConfig")
            .field("tokens", &self.token_digests.len())
            .field("allowed_ips", &self.allowed_ips)
            .field("trusted_proxies", &self.trusted_proxies)
            .finish()
    }
}

/// An IP address or CIDR range such as `10.0.0.5`, `10.0.0.0/24` or `fd00::/8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpCidr {
    addr: IpAddr,
    prefix: u8,
}

impl IpCidr {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let value = value.trim();
        let (addr, prefix) = match value.split_once('/') {
            Some((addr, prefix)) => (addr, Some(prefix)),
            None => (value, None),
        };
        let addr = addr
            .parse::<IpAddr>()
            .with_context(|| format!("'{value}' is not an IP address or CIDR range"))?
            .to_canonical();
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(prefix) => prefix
                .parse::<u8>()
                .ok()
                .filter(|prefix| *prefix <= max)
                .with_context(|| format!("'{value}' has an invalid prefix length"))?,
            None => max,
        };
        Ok(Self { addr, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip.to_canonical()) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
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
    pub auto_payment_reminder: Option<AutoPaymentReminderConfig>,
    pub timezone: String,
    pub session_ttl: Duration,
    /// Folder with private avatar artwork (team photos, stickers), served only
    /// to members through `/api/v1/cosmetics/artwork/{id}`. Never published.
    pub cosmetics_private_dir: String,
    /// `None` disables `/api/v1/external/*` (it then answers 404).
    pub external_api: Option<ExternalApiConfig>,
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
        let auto_payment_reminder = build_auto_payment_reminder_config(
            optional_env("AUTO_PAYMENT_REMINDER_LINK_USER_ID"),
            optional_env("AUTO_PAYMENT_REMINDER_HOUR"),
        )?;
        let timezone = env::var("TIMEZONE").unwrap_or_else(|_| "Europe/Berlin".into());
        let external_api = build_external_api_config(
            optional_env("EXTERNAL_API_TOKEN"),
            optional_env("EXTERNAL_API_ALLOWED_IPS"),
            optional_env("EXTERNAL_API_TRUSTED_PROXIES"),
        )?;

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
            auto_payment_reminder,
            timezone,
            session_ttl: Duration::from_secs(60 * 60 * 24 * 180),
            cosmetics_private_dir: env::var("COSMETICS_PRIVATE_DIR")
                .unwrap_or_else(|_| "../cosmetics_private".into()),
            external_api,
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

fn build_auto_payment_reminder_config(
    payment_link_user_id: Option<String>,
    hour: Option<String>,
) -> anyhow::Result<Option<AutoPaymentReminderConfig>> {
    let Some(payment_link_user_id) = payment_link_user_id else {
        return Ok(None);
    };
    let hour = hour
        .unwrap_or_else(|| "10".into())
        .parse::<u32>()
        .ok()
        .filter(|hour| *hour < 24)
        .with_context(|| "AUTO_PAYMENT_REMINDER_HOUR must be an hour between 0 and 23")?;
    Ok(Some(AutoPaymentReminderConfig {
        payment_link_user_id,
        hour,
    }))
}

fn build_external_api_config(
    tokens: Option<String>,
    allowed_ips: Option<String>,
    trusted_proxies: Option<String>,
) -> anyhow::Result<Option<ExternalApiConfig>> {
    let Some(tokens) = tokens else {
        if allowed_ips.is_some() || trusted_proxies.is_some() {
            tracing::warn!("EXTERNAL_API_TOKEN is not set, so the external API stays disabled");
        }
        return Ok(None);
    };
    let tokens = parse_csv_env(tokens);
    if tokens.is_empty() {
        return Ok(None);
    }
    if tokens
        .iter()
        .any(|token| token.len() < EXTERNAL_API_TOKEN_MIN_LEN)
    {
        bail!(
            "every EXTERNAL_API_TOKEN entry must be at least {EXTERNAL_API_TOKEN_MIN_LEN} characters \
             (generate one with `openssl rand -hex 32`)"
        );
    }
    let token_digests = tokens
        .iter()
        .map(|token| Sha256::digest(token.as_bytes()).into())
        .collect();
    let parse_ips = |name: &str, value: Option<String>| -> anyhow::Result<Vec<IpCidr>> {
        parse_csv_env(value.unwrap_or_default())
            .iter()
            .map(|entry| IpCidr::parse(entry).with_context(|| format!("invalid {name}")))
            .collect()
    };
    Ok(Some(ExternalApiConfig {
        token_digests,
        allowed_ips: parse_ips("EXTERNAL_API_ALLOWED_IPS", allowed_ips)?,
        trusted_proxies: parse_ips("EXTERNAL_API_TRUSTED_PROXIES", trusted_proxies)?,
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
    use super::{
        IpCidr, SmtpSecurity, build_auto_payment_reminder_config, build_external_api_config,
        build_smtp_config, non_empty_trimmed, parse_csv_env,
    };

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

    #[test]
    fn auto_payment_reminder_is_disabled_without_link_user() {
        assert_eq!(
            build_auto_payment_reminder_config(None, Some("8".into())).unwrap(),
            None
        );
    }

    #[test]
    fn auto_payment_reminder_hour_defaults_to_ten_and_is_validated() {
        let config = build_auto_payment_reminder_config(Some("staff".into()), None)
            .unwrap()
            .unwrap();
        assert_eq!(config.payment_link_user_id, "staff");
        assert_eq!(config.hour, 10);

        let config = build_auto_payment_reminder_config(Some("staff".into()), Some("0".into()))
            .unwrap()
            .unwrap();
        assert_eq!(config.hour, 0);

        assert!(
            build_auto_payment_reminder_config(Some("staff".into()), Some("24".into())).is_err()
        );
        assert!(
            build_auto_payment_reminder_config(Some("staff".into()), Some("ten".into())).is_err()
        );
    }

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn external_api_is_disabled_without_a_token() {
        assert_eq!(build_external_api_config(None, None, None).unwrap(), None);
        assert_eq!(
            build_external_api_config(None, Some("10.0.0.5".into()), None).unwrap(),
            None
        );
    }

    #[test]
    fn external_api_keeps_only_token_digests_and_parses_ip_lists() {
        let config = build_external_api_config(
            Some(format!("{TOKEN}, {TOKEN}-rotated")),
            Some("10.0.0.5, 192.168.0.0/16, fd00::/8".into()),
            Some("172.16.0.0/12".into()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(config.token_digests.len(), 2);
        assert_eq!(config.allowed_ips.len(), 3);
        assert_eq!(config.trusted_proxies.len(), 1);
        assert!(!format!("{config:?}").contains(TOKEN));
    }

    #[test]
    fn external_api_rejects_short_tokens_and_bad_ip_lists() {
        assert!(build_external_api_config(Some("short".into()), None, None).is_err());
        assert!(build_external_api_config(Some(format!("{TOKEN},short")), None, None).is_err());
        assert!(
            build_external_api_config(Some(TOKEN.into()), Some("10.0.0.300".into()), None).is_err()
        );
        assert!(
            build_external_api_config(Some(TOKEN.into()), None, Some("10.0.0.0/33".into()))
                .is_err()
        );
    }

    #[test]
    fn ip_cidr_matches_addresses_and_ranges() {
        let single = IpCidr::parse("10.0.0.5").unwrap();
        assert!(single.contains("10.0.0.5".parse().unwrap()));
        assert!(!single.contains("10.0.0.6".parse().unwrap()));

        let range = IpCidr::parse("192.168.0.0/16").unwrap();
        assert!(range.contains("192.168.42.1".parse().unwrap()));
        assert!(range.contains("::ffff:192.168.42.1".parse().unwrap()));
        assert!(!range.contains("192.169.0.1".parse().unwrap()));
        assert!(!range.contains("fd00::1".parse().unwrap()));

        let v6 = IpCidr::parse("fd00::/8").unwrap();
        assert!(v6.contains("fd12:3456::1".parse().unwrap()));
        assert!(!v6.contains("fe80::1".parse().unwrap()));

        let any = IpCidr::parse("0.0.0.0/0").unwrap();
        assert!(any.contains("8.8.8.8".parse().unwrap()));

        assert!(IpCidr::parse("not-an-ip").is_err());
        assert!(IpCidr::parse("10.0.0.0/x").is_err());
    }
}
