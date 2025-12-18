use crate::RoutingPolicy;
use reqwest::{Client, Method, Request, RequestBuilder, Response, Url};
use std::collections::HashSet;
use thiserror::Error;
use url::ParseError;

const FORBIDDEN_HOST_CHARS: &[char] = &['\u{ff0e}', '\u{3002}', '\u{ff61}', '\u{fe52}'];

const KEY_EGRESS: &str = "egress";
const KEY_DEFAULT: &str = "default";
const KEY_ALLOW: &str = "allow";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AllowedTarget {
    scheme: Option<String>,
    host: String,
    port: Option<u16>,
}

impl AllowedTarget {
    fn new(scheme: Option<&str>, host: &str, port: Option<u16>) -> Self {
        Self {
            scheme: scheme.map(str::to_ascii_lowercase),
            host: normalize_host(host),
            port,
        }
    }
}

#[derive(Debug, Error)]
pub enum AllowEntryError {
    #[error(transparent)]
    Url(#[from] ParseError),
    #[error("missing host component")]
    MissingHost,
    #[error("host contains forbidden characters")]
    InvalidHost,
}

fn parse_allow_entry(entry: &str) -> Result<AllowedTarget, AllowEntryError> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Err(AllowEntryError::MissingHost);
    }

    if let Ok(url) = Url::parse(trimmed) {
        if url.host_str().is_some() {
            return allowed_target_from_url(&url, true, Some(url.scheme()));
        }

        if trimmed.contains("://") {
            return Err(AllowEntryError::MissingHost);
        }
    }

    let fallback = format!("http://{trimmed}");
    let url = Url::parse(&fallback).map_err(AllowEntryError::Url)?;
    allowed_target_from_url(&url, false, None)
}

fn normalize_host(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

fn allowed_target_from_url(
    url: &Url,
    had_scheme: bool,
    explicit_scheme: Option<&str>,
) -> Result<AllowedTarget, AllowEntryError> {
    let host = url.host_str().ok_or(AllowEntryError::MissingHost)?;
    if host_contains_forbidden_chars(host) {
        return Err(AllowEntryError::InvalidHost);
    }
    let port = if let Some(port) = url.port() {
        Some(port)
    } else if had_scheme {
        url.port_or_known_default()
    } else {
        None
    };

    Ok(AllowedTarget::new(explicit_scheme, host, port))
}

fn raw_host_segment(url: &str) -> Option<&str> {
    let (_, remainder) = url.split_once("://")?;
    let end = remainder
        .char_indices()
        .find(|(_, ch)| matches!(ch, '/' | '?' | '#'))
        .map_or_else(|| remainder.len(), |(idx, _)| idx);
    let authority = &remainder[..end];
    if authority.is_empty() {
        return None;
    }

    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host_port.is_empty() {
        return None;
    }

    if let Some(stripped) = strip_port(host_port) {
        Some(stripped)
    } else {
        Some(host_port)
    }
}

fn strip_port(host_port: &str) -> Option<&str> {
    if host_port.starts_with('[') {
        host_port.find(']').map(|idx| &host_port[..=idx])
    } else {
        host_port.rsplit_once(':').map(|(host, candidate_port)| {
            if candidate_port.chars().all(|c| c.is_ascii_digit()) {
                host
            } else {
                host_port
            }
        })
    }
}

fn host_contains_forbidden_chars(host: &str) -> bool {
    host.chars()
        .any(|c| c.is_ascii_control() || c.is_ascii_whitespace())
        || host.contains('%')
        || host.chars().any(|c| FORBIDDEN_HOST_CHARS.contains(&c))
        || host.ends_with('.')
}

#[derive(Debug, Error)]
pub enum EgressGuardError {
    #[error("egress section must be a mapping")]
    InvalidEgressSection,
    #[error("egress.default must be 'allow' or 'deny', got '{0}'")]
    UnknownDefault(String),
    #[error("egress.allow must be a sequence of strings")]
    InvalidAllowList,
    #[error("invalid host in allow entry '{entry}': {source}")]
    InvalidAllowHost {
        entry: String,
        source: AllowEntryError,
    },
}

#[derive(Debug, Error)]
pub enum GuardError {
    #[error("failed to parse URL: {0}")]
    InvalidUrl(#[from] ParseError),
    #[error("URL has no host component")]
    MissingHost,
    #[error("egress denied for host '{host}'")]
    HostDenied { host: String },
}

#[derive(Debug, Error)]
pub enum GuardedRequestError {
    #[error(transparent)]
    Guard(#[from] GuardError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct EgressGuard {
    enforce: bool,
    allowed: HashSet<AllowedTarget>,
}

impl Default for EgressGuard {
    fn default() -> Self {
        Self::allow_all()
    }
}

impl EgressGuard {
    pub fn allow_all() -> Self {
        Self {
            enforce: false,
            allowed: HashSet::new(),
        }
    }

    pub fn is_enforced(&self) -> bool {
        self.enforce
    }

    pub fn ensure_allowed(&self, url: &str) -> Result<Url, GuardError> {
        if let Some(host) = raw_host_segment(url) {
            if host_contains_forbidden_chars(host) {
                return Err(GuardError::HostDenied {
                    host: host.to_string(),
                });
            }
        }

        let parsed = Url::parse(url)?;
        self.ensure_url_is_allowed(&parsed)?;
        Ok(parsed)
    }

    pub fn from_policy(policy: &RoutingPolicy) -> Result<Self, EgressGuardError> {
        let mapping = match policy.0.as_mapping() {
            Some(mapping) => mapping,
            None => return Ok(Self::allow_all()),
        };

        let Some(egress_value) = mapping.get(serde_yaml_ng::Value::from(KEY_EGRESS)) else {
            return Ok(Self::allow_all());
        };

        let egress_map = egress_value
            .as_mapping()
            .ok_or(EgressGuardError::InvalidEgressSection)?;

        let default_action = egress_map
            .get(serde_yaml_ng::Value::from(KEY_DEFAULT))
            .and_then(|value| value.as_str())
            .unwrap_or("allow")
            .to_ascii_lowercase();

        let enforce = match default_action.as_str() {
            "allow" => false,
            "deny" => true,
            other => return Err(EgressGuardError::UnknownDefault(other.to_string())),
        };

        let mut allowed = HashSet::new();
        if let Some(allow_value) = egress_map.get(serde_yaml_ng::Value::from(KEY_ALLOW)) {
            let allow_seq = allow_value
                .as_sequence()
                .ok_or(EgressGuardError::InvalidAllowList)?;
            for entry in allow_seq {
                let entry = entry
                    .as_str()
                    .ok_or(EgressGuardError::InvalidAllowList)?
                    .trim();
                let target = if entry.contains("://") {
                    let url =
                        Url::parse(entry).map_err(|e| EgressGuardError::InvalidAllowHost {
                            entry: entry.to_string(),
                            source: AllowEntryError::Url(e),
                        })?;
                    allowed_target_from_url(&url, true, Some(url.scheme())).map_err(|source| {
                        EgressGuardError::InvalidAllowHost {
                            entry: entry.to_string(),
                            source,
                        }
                    })?
                } else {
                    parse_allow_entry(entry).map_err(|source| {
                        EgressGuardError::InvalidAllowHost {
                            entry: entry.to_string(),
                            source,
                        }
                    })?
                };
                allowed.insert(target);
            }
        }

        Ok(Self { enforce, allowed })
    }

    fn ensure_url_is_allowed(&self, url: &Url) -> Result<(), GuardError> {
        if !self.enforce {
            return Ok(());
        }

        let host = url.host_str().ok_or(GuardError::MissingHost)?;
        if host_contains_forbidden_chars(host) {
            return Err(GuardError::HostDenied {
                host: host.to_string(),
            });
        }
        let normalized_host = normalize_host(host);

        if !url.username().is_empty() || url.password().is_some() {
            return Err(GuardError::HostDenied {
                host: normalized_host.clone(),
            });
        }

        let host_any_port = AllowedTarget::new(Some(url.scheme()), host, None);
        if self.allowed.contains(&host_any_port) {
            return Ok(());
        }

        let host_any_port_neutral = AllowedTarget::new(None, host, None);
        if self.allowed.contains(&host_any_port_neutral) {
            return Ok(());
        }

        if let Some(port) = url.port_or_known_default() {
            let host_with_port = AllowedTarget::new(Some(url.scheme()), host, Some(port));
            if self.allowed.contains(&host_with_port) {
                return Ok(());
            }

            let host_with_port_neutral = AllowedTarget::new(None, host, Some(port));
            if self.allowed.contains(&host_with_port_neutral) {
                return Ok(());
            }
        }

        let display = match url.port_or_known_default() {
            Some(port) => format!("{normalized_host}:{port}"),
            None => normalized_host.clone(),
        };
        Err(GuardError::HostDenied { host: display })
    }
}

#[derive(Clone, Debug)]
pub struct AllowlistedClient {
    inner: Client,
    guard: EgressGuard,
}

impl AllowlistedClient {
    pub fn new(inner: Client, guard: EgressGuard) -> Self {
        Self { inner, guard }
    }

    pub fn from_routing_policy(
        inner: Client,
        policy: &RoutingPolicy,
    ) -> Result<Self, EgressGuardError> {
        let guard = EgressGuard::from_policy(policy)?;
        Ok(Self::new(inner, guard))
    }

    pub fn guard(&self) -> &EgressGuard {
        &self.guard
    }

    pub fn client(&self) -> &Client {
        &self.inner
    }

    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder, GuardError> {
        let url = self.guard.ensure_allowed(url)?;
        self.request_url(method, url)
    }

    pub fn request_url(&self, method: Method, url: Url) -> Result<RequestBuilder, GuardError> {
        self.guard.ensure_url_is_allowed(&url)?;
        Ok(self.inner.request(method, url))
    }

    pub fn get(&self, url: &str) -> Result<RequestBuilder, GuardError> {
        self.request(Method::GET, url)
    }

    pub fn post(&self, url: &str) -> Result<RequestBuilder, GuardError> {
        self.request(Method::POST, url)
    }

    pub fn put(&self, url: &str) -> Result<RequestBuilder, GuardError> {
        self.request(Method::PUT, url)
    }

    pub fn delete(&self, url: &str) -> Result<RequestBuilder, GuardError> {
        self.request(Method::DELETE, url)
    }

    pub async fn execute(&self, request: Request) -> Result<Response, GuardedRequestError> {
        self.guard.ensure_url_is_allowed(request.url())?;
        Ok(self.inner.execute(request).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_from_yaml(yaml: &str) -> RoutingPolicy {
        RoutingPolicy(serde_yaml_ng::from_str(yaml).unwrap())
    }

    #[test]
    fn guard_not_enforced_when_default_allow() {
        let policy = policy_from_yaml(
            r"
routing:
  prefer_local: true
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        assert!(!guard.is_enforced());
    }

    #[test]
    fn guard_enforced_when_default_deny() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
    - internal.service:8443
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        assert!(guard.is_enforced());

        guard
            .ensure_url_is_allowed(&Url::parse("https://api.matrix.example/v1").unwrap())
            .unwrap();
        guard
            .ensure_url_is_allowed(&Url::parse("https://internal.service:8443/health").unwrap())
            .unwrap();
        assert!(guard
            .ensure_url_is_allowed(&Url::parse("https://evil.example").unwrap())
            .is_err());
    }

    #[test]
    fn guard_rejects_mismatched_port_for_scheme_entries() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();

        guard
            .ensure_url_is_allowed(&Url::parse("https://api.matrix.example/v1").unwrap())
            .unwrap();
        assert!(matches!(
            guard
                .ensure_url_is_allowed(&Url::parse("https://api.matrix.example:8443/alt").unwrap()),
            Err(GuardError::HostDenied { .. })
        ));
    }

    #[test]
    fn guard_supports_host_only_entries() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - metrics.internal
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        guard
            .ensure_url_is_allowed(&Url::parse("http://metrics.internal/data").unwrap())
            .unwrap();
    }

    #[test]
    fn guard_rejects_unknown_default_action() {
        let policy = policy_from_yaml(
            r"
egress:
  default: maybe
",
        );
        let err = EgressGuard::from_policy(&policy).unwrap_err();
        match err {
            EgressGuardError::UnknownDefault(value) => assert_eq!(value, "maybe"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn guard_rejects_common_evasion_patterns() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();

        for candidate in [
            "https://evil.api.matrix.example",
            "https://api.matrix.example@evil.com",
            "https://user:pass@api.matrix.example",
            "https://арi.matrix.example",
            "https://api.matrix.example:4443",
            "https://api.matrix.example.evil.com",
            "https://API.Matrix.Example./",
            "https://api\u{ff0e}matrix\u{ff0e}example",
            "https://api.matrix.example%20",
            "https://api.matrix.example\n.evil",
        ] {
            assert!(
                guard.ensure_allowed(candidate).is_err(),
                "candidate {candidate} should be denied"
            );
        }

        guard.ensure_allowed("https://api.matrix.example").unwrap();
        guard
            .ensure_allowed("https://api.matrix.example:443")
            .unwrap();
    }

    #[test]
    fn guard_allows_ipv6_targets_with_explicit_port() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://[2001:db8::1]:8443
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        guard
            .ensure_allowed("https://[2001:db8::1]:8443/status")
            .unwrap();
        assert!(matches!(
            guard.ensure_allowed("https://[2001:db8::1]:443/status"),
            Err(GuardError::HostDenied { .. })
        ));
    }

    #[test]
    fn guard_rejects_http_when_only_https_allowed() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        guard.ensure_allowed("https://api.matrix.example").unwrap();
        assert!(matches!(
            guard.ensure_allowed("http://api.matrix.example"),
            Err(GuardError::HostDenied { .. })
        ));
    }

    #[test]
    fn allowlisted_client_blocks_disallowed_hosts() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        let client = AllowlistedClient::new(Client::new(), guard);
        let err = client.get("https://disallowed.example").unwrap_err();
        match err {
            GuardError::HostDenied { host } => assert!(host.contains("disallowed.example")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn allowlisted_client_allows_permitted_host() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        let client = AllowlistedClient::new(Client::new(), guard);
        let request = client
            .get("https://api.matrix.example/v1")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.url().host_str(), Some("api.matrix.example"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_blocks_disallowed_request() {
        let policy = policy_from_yaml(
            r"
egress:
  default: deny
  allow:
    - https://api.matrix.example
",
        );
        let guard = EgressGuard::from_policy(&policy).unwrap();
        let reqwest_client = Client::new();
        let client = AllowlistedClient::new(reqwest_client.clone(), guard);
        let request = reqwest_client.get("https://evil.example").build().unwrap();
        let err = client.execute(request).await.unwrap_err();
        match err {
            GuardedRequestError::Guard(GuardError::HostDenied { host }) => {
                assert!(host.contains("evil.example"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
