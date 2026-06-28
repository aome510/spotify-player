use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

use crate::config;
use anyhow::{Context as _, Result};
use base64::Engine as _;
use librespot_core::{authentication::Credentials, cache::Cache, config::SessionConfig, Session};
use reqwest::Url;
use rspotify::clients::{BaseClient as _, OAuthClient as _};
use sha2::{Digest as _, Sha256};

pub const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
pub const NCSPOT_CLIENT_ID: &str = "d420a117a32841c2b3474932e49fb54b";

const SPOTIFY_AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const SPOTIFY_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

// based on https://developer.spotify.com/documentation/web-api/concepts/scopes#list-of-scopes
pub const OAUTH_SCOPES: &[&str] = &[
    // Spotify Connect
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    // Playback
    "app-remote-control",
    "streaming",
    // Playlists
    "playlist-read-private",
    "playlist-read-collaborative",
    "playlist-modify-private",
    "playlist-modify-public",
    // Follow
    "user-follow-modify",
    "user-follow-read",
    // Listening History
    "user-read-playback-position",
    "user-top-read",
    "user-read-recently-played",
    // Library
    "user-library-modify",
    "user-library-read",
    // Users
    "user-personalized",
];

#[derive(Clone)]
pub struct AuthConfig {
    pub cache: Cache,
    pub session_config: SessionConfig,
    pub login_redirect_uri: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            cache: Cache::new(None::<String>, None, None, None).unwrap(),
            session_config: SessionConfig::default(),
            login_redirect_uri: "http://127.0.0.1:8989/login".to_string(),
        }
    }
}

impl AuthConfig {
    /// Create a `librespot::Session` from authentication configs
    pub fn session(&self) -> Session {
        Session::new(self.session_config.clone(), Some(self.cache.clone()))
    }

    pub fn new(configs: &config::Configs) -> Result<AuthConfig> {
        let audio_cache_folder = if configs.app_config.device.audio_cache {
            Some(configs.cache_folder.join("audio"))
        } else {
            None
        };

        let cache = Cache::new(
            Some(configs.cache_folder.clone()),
            None,
            audio_cache_folder,
            None,
        )?;

        Ok(AuthConfig {
            cache,
            session_config: configs.app_config.session_config(),
            login_redirect_uri: configs.app_config.login_redirect_uri.clone(),
        })
    }
}

/// Get Spotify credentials to authenticate the application
///
/// # Args
/// - `auth_config`: authentication configuration
/// - `reauth`: whether to re-authenticate the application if no cached credentials are found
// - `use_cached`: whether to use cached credentials if available
pub fn get_creds(auth_config: &AuthConfig, reauth: bool, use_cached: bool) -> Result<Credentials> {
    let creds = if use_cached {
        auth_config.cache.credentials()
    } else {
        None
    };

    Ok(match creds {
        None => {
            let msg = "No cached credentials found, please authenticate the application first.";
            if reauth {
                eprintln!("{msg}");

                let access_token = get_oauth_access_token(
                    SPOTIFY_CLIENT_ID,
                    &auth_config.login_redirect_uri,
                    OAUTH_SCOPES,
                )?;
                Credentials::with_access_token(access_token)
            } else {
                anyhow::bail!(msg);
            }
        }
        Some(creds) => {
            tracing::info!("Using cached credentials");
            creds
        }
    })
}

/// Authenticate the user-provided (Web API) client using the authorization code with PKCE flow.
///
/// This mirrors `rspotify`'s `prompt_for_token` (reusing/refreshing a cached token when possible),
/// but replaces its callback listener with [`obtain_auth_code`], which is robust against stray
/// browser requests on the callback port (see [`listen_for_auth_code`]).
pub async fn prompt_for_user_token(client: &mut rspotify::AuthCodePkceSpotify) -> Result<()> {
    // Reuse a cached token when possible, refreshing it if it has expired.
    if let Ok(Some(token)) = client.read_token_cache(true).await {
        let expired = token.is_expired();
        *client.get_token().lock().await.unwrap() = Some(token);

        if !expired {
            return Ok(());
        }

        if let Some(refreshed) = client
            .refetch_token()
            .await
            .context("refresh expired token from cache")?
        {
            *client.get_token().lock().await.unwrap() = Some(refreshed);
            client
                .write_token_cache()
                .await
                .context("write refreshed token to cache")?;
            return Ok(());
        }
    }

    // No usable cached token: run the interactive authorization code flow.
    // `get_authorize_url` also generates and stores the PKCE verifier used by `request_token`.
    let url = client
        .get_authorize_url(None)
        .context("get authorize URL for user-provided client")?;
    let code = obtain_auth_code(&url, &client.get_oauth().redirect_uri)?;
    client
        .request_token(&code)
        .await
        .context("exchange auth code for token (user-provided client)")?;

    Ok(())
}

/// Run the authorization code with PKCE flow for `librespot` and return an access token.
fn get_oauth_access_token(client_id: &str, redirect_uri: &str, scopes: &[&str]) -> Result<String> {
    let pkce = Pkce::new_random();
    let state = random_url_safe(16);
    let auth_url = build_authorize_url(client_id, redirect_uri, scopes, &pkce.challenge, &state)?;

    let code = obtain_auth_code(auth_url.as_str(), redirect_uri)?;
    exchange_code_for_token(client_id, redirect_uri, &code, &pkce.verifier)
}

/// Open the authorization URL in a browser and obtain the auth `code` from the redirect.
///
/// If `redirect_uri` is an HTTP loopback address with a port, a local server collects the code
/// automatically; otherwise the user is prompted to paste the redirect URL on stdin.
fn obtain_auth_code(auth_url: &str, redirect_uri: &str) -> Result<String> {
    open::that_in_background(auth_url);
    println!("Browse to: {auth_url}");

    match redirect_socket_address(redirect_uri) {
        Some(addr) => listen_for_auth_code(addr),
        None => read_auth_code_from_stdin(),
    }
}

/// Spawn a local HTTP server that waits for the OAuth redirect and returns the auth `code`.
///
/// Browsers commonly prefetch resources such as `/favicon.ico` or
/// `/apple-touch-icon-precomposed.png` from the callback server. Unlike the listeners shipped by
/// `librespot-oauth` and `rspotify` — which treat the *first* incoming connection as the redirect
/// and therefore fail with "Auth code param not found" when a prefetch arrives first — this server
/// ignores any request that does not carry an auth `code` and keeps listening until the real
/// redirect arrives.
fn listen_for_auth_code(addr: SocketAddr) -> Result<String> {
    let listener =
        TcpListener::bind(addr).with_context(|| format!("bind OAuth callback server to {addr}"))?;
    tracing::info!("OAuth callback server listening on {addr}");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                tracing::warn!("Failed to accept an OAuth callback connection: {err:#}");
                continue;
            }
        };

        let mut request_line = String::new();
        if let Err(err) = BufReader::new(&stream).read_line(&mut request_line) {
            tracing::warn!("Failed to read an OAuth callback request: {err:#}");
            continue;
        }

        // The request line looks like `GET /login?code=...&state=... HTTP/1.1`.
        let request_target = request_line.split_whitespace().nth(1).unwrap_or_default();
        if let Some(code) = code_from_redirect(request_target) {
            respond(
                &mut stream,
                "200 OK",
                "Authentication successful! You can close this tab and go back to your terminal.",
            );
            return Ok(code);
        }

        // Ignore stray requests (e.g. favicon / apple-touch-icon prefetches) that don't carry an
        // auth code, and keep listening for the real redirect.
        tracing::debug!("Ignoring OAuth callback request without an auth code: {request_target}");
        respond(&mut stream, "404 Not Found", "");
    }

    anyhow::bail!("OAuth callback server stopped before receiving an auth code");
}

/// Prompt for the redirect URL on stdin and extract the auth `code`.
fn read_auth_code_from_stdin() -> Result<String> {
    println!("Enter the URL you were redirected to: ");
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .context("read redirect URL from stdin")?;
    code_from_redirect(buffer.trim()).context("no auth code found in the provided redirect URL")
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(err) = stream.write_all(response.as_bytes()) {
        tracing::warn!("Failed to write an OAuth callback response: {err:#}");
    }
}

/// Extract the `code` query parameter from a redirect, accepting either a full URL or a bare
/// request target (e.g. `/login?code=...`).
fn code_from_redirect(redirect: &str) -> Option<String> {
    let url = Url::parse(redirect)
        .or_else(|_| Url::parse(&format!("http://localhost{redirect}")))
        .ok()?;
    url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, code)| code.into_owned())
}

/// Resolve the loopback socket address that an `http://host:port/...` redirect URI listens on.
fn redirect_socket_address(redirect_uri: &str) -> Option<SocketAddr> {
    let url = match Url::parse(redirect_uri) {
        Ok(url) if url.scheme() == "http" && url.port().is_some() => url,
        _ => return None,
    };
    url.socket_addrs(|| None).ok()?.into_iter().next()
}

fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scopes: &[&str],
    challenge: &str,
    state: &str,
) -> Result<Url> {
    let scope = scopes.join(" ");
    Url::parse_with_params(
        SPOTIFY_AUTHORIZE_URL,
        &[
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", scope.as_str()),
            ("code_challenge_method", "S256"),
            ("code_challenge", challenge),
            ("state", state),
        ],
    )
    .context("build Spotify authorize URL")
}

/// Exchange an authorization code for an access token via Spotify's token endpoint.
fn exchange_code_for_token(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", verifier),
    ];

    // `reqwest::blocking` spins up its own runtime, which panics if constructed on a thread that is
    // already running one. `get_creds` may be called from within the async client task, so perform
    // the exchange on a dedicated thread.
    std::thread::scope(|s| {
        s.spawn(|| {
            let token = reqwest::blocking::Client::new()
                .post(SPOTIFY_TOKEN_URL)
                .form(&params)
                .send()
                .context("send token exchange request")?
                .error_for_status()
                .context("token exchange request failed")?
                .json::<TokenResponse>()
                .context("parse token exchange response")?;
            Ok(token.access_token)
        })
        .join()
        .map_err(|_| anyhow::anyhow!("token exchange thread panicked"))?
    })
}

/// A PKCE code verifier/challenge pair (RFC 7636).
struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    fn new_random() -> Self {
        let verifier = random_url_safe(32);
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// Generate a random URL-safe (base64url, no padding) string from `n` random bytes.
fn random_url_safe(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::fill(bytes.as_mut_slice());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn code_from_redirect_extracts_code() {
        // Bare request target (as read from the HTTP request line) and full URL both work.
        assert_eq!(
            code_from_redirect("/login?code=abc123&state=xyz").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            code_from_redirect("http://127.0.0.1:8989/login?code=abc123&state=xyz").as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn code_from_redirect_ignores_stray_requests() {
        // The exact request that previously broke authentication: a browser prefetch with no code.
        assert_eq!(
            code_from_redirect("/apple-touch-icon-precomposed.png"),
            None
        );
        assert_eq!(code_from_redirect("/favicon.ico"), None);
        assert_eq!(code_from_redirect("/login"), None);
    }

    #[test]
    fn redirect_socket_address_requires_http_and_port() {
        assert!(redirect_socket_address("http://127.0.0.1:8989/login").is_some());
        // No port / non-http schemes fall back to the stdin flow.
        assert!(redirect_socket_address("http://127.0.0.1/login").is_none());
        assert!(redirect_socket_address("https://127.0.0.1:8989/login").is_none());
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        // Verifier/challenge pair from RFC 7636, Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}
