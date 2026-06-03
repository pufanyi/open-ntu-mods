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
    pub email_login_enabled: bool,
    pub email_login_delivery: String,
    pub email_login_allowed_domains: Vec<String>,
    pub email_from: Option<String>,
    pub resend_api_key: Option<String>,
    pub ntu_allowed_domains: Vec<String>,
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
            ntu_allowed_domains: env::var("NTU_ALLOWED_DOMAINS")
                .unwrap_or_else(|_| "e.ntu.edu.sg,ntu.edu.sg".to_string())
                .split(',')
                .map(|domain| domain.trim().trim_start_matches('@').to_ascii_lowercase())
                .filter(|domain| !domain.is_empty())
                .collect(),
            email_login_enabled: parse_bool("EMAIL_LOGIN_ENABLED", true),
            email_login_delivery: env::var("EMAIL_LOGIN_DELIVERY")
                .unwrap_or_else(|_| "log".to_string())
                .trim()
                .to_ascii_lowercase(),
            email_login_allowed_domains: parse_domains_with_fallback(
                "EMAIL_LOGIN_ALLOWED_DOMAINS",
                "e.ntu.edu.sg,ntu.edu.sg",
            ),
            email_from: optional_env("EMAIL_FROM"),
            resend_api_key: optional_env("RESEND_API_KEY"),
            enable_dev_login: parse_bool("ENABLE_DEV_LOGIN", false),
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
        })
    }

    pub fn email_domain_allowed(&self, email: &str) -> bool {
        domain_allowed(email, &self.ntu_allowed_domains)
    }

    pub fn email_login_domain_allowed(&self, email: &str) -> bool {
        domain_allowed(email, &self.email_login_allowed_domains)
    }
}

fn domain_allowed(email: &str, allowed_domains: &[String]) -> bool {
    if allowed_domains.iter().any(|allowed| allowed == "*") {
        return true;
    }

    let Some(domain) = email.rsplit_once('@').map(|(_, domain)| domain) else {
        return false;
    };
    let domain = domain.to_ascii_lowercase();
    allowed_domains
        .iter()
        .any(|allowed| domain == *allowed || domain.ends_with(&format!(".{allowed}")))
}

fn parse_domains_with_fallback(name: &str, fallback: &str) -> Vec<String> {
    env::var(name)
        .unwrap_or_else(|_| {
            env::var("NTU_ALLOWED_DOMAINS").unwrap_or_else(|_| fallback.to_string())
        })
        .split(',')
        .map(|domain| domain.trim().trim_start_matches('@').to_ascii_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect()
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

#[cfg(test)]
mod tests {
    use super::domain_allowed;

    #[test]
    fn domain_allowlist_supports_wildcard_exact_and_subdomains() {
        assert!(domain_allowed("user@example.com", &[String::from("*")]));
        assert!(domain_allowed(
            "user@e.ntu.edu.sg",
            &[String::from("e.ntu.edu.sg")]
        ));
        assert!(domain_allowed(
            "user@mail.e.ntu.edu.sg",
            &[String::from("e.ntu.edu.sg")]
        ));
        assert!(!domain_allowed(
            "user@gmail.com",
            &[String::from("e.ntu.edu.sg")]
        ));
    }
}
