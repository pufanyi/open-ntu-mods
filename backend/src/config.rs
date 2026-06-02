use std::{env, net::SocketAddr};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub app_public_url: String,
    pub backend_public_url: String,
    pub session_secret: String,
    pub cookie_secure: bool,
    pub require_origin_secret: bool,
    pub origin_secret: String,
    pub run_migrations_on_startup: bool,
    pub microsoft_client_id: Option<String>,
    pub microsoft_client_secret: Option<String>,
    pub microsoft_issuer: String,
    pub ntu_allowed_domains: Vec<String>,
    pub ntu_tenant_id: Option<String>,
    pub enable_dev_login: bool,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(3000);

        Ok(Self {
            database_url: required_env("DATABASE_URL")?,
            app_public_url: env::var("APP_PUBLIC_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            backend_public_url: env::var("BACKEND_PUBLIC_URL")
                .unwrap_or_else(|_| format!("http://localhost:{port}")),
            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-me".to_string()),
            cookie_secure: parse_bool("COOKIE_SECURE", false),
            require_origin_secret: parse_bool("REQUIRE_ORIGIN_SECRET", false),
            origin_secret: env::var("ORIGIN_SECRET")
                .unwrap_or_else(|_| "dev-origin-secret".to_string()),
            run_migrations_on_startup: parse_bool("RUN_MIGRATIONS_ON_STARTUP", false),
            microsoft_client_id: optional_env("MICROSOFT_CLIENT_ID"),
            microsoft_client_secret: optional_env("MICROSOFT_CLIENT_SECRET"),
            microsoft_issuer: env::var("MICROSOFT_ISSUER").unwrap_or_else(|_| {
                "https://login.microsoftonline.com/organizations/v2.0".to_string()
            }),
            ntu_allowed_domains: env::var("NTU_ALLOWED_DOMAINS")
                .unwrap_or_else(|_| "e.ntu.edu.sg,ntu.edu.sg".to_string())
                .split(',')
                .map(|domain| domain.trim().trim_start_matches('@').to_ascii_lowercase())
                .filter(|domain| !domain.is_empty())
                .collect(),
            ntu_tenant_id: optional_env("NTU_TENANT_ID"),
            enable_dev_login: parse_bool("ENABLE_DEV_LOGIN", false),
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
        })
    }

    pub fn microsoft_redirect_uri(&self) -> String {
        format!(
            "{}/auth/microsoft/callback",
            self.backend_public_url.trim_end_matches('/')
        )
    }

    pub fn email_domain_allowed(&self, email: &str) -> bool {
        let Some(domain) = email.rsplit_once('@').map(|(_, domain)| domain) else {
            return false;
        };
        let domain = domain.to_ascii_lowercase();
        self.ntu_allowed_domains
            .iter()
            .any(|allowed| domain == *allowed || domain.ends_with(&format!(".{allowed}")))
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("{name} must be set"))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}
