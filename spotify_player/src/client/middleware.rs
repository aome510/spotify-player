use std::{collections::HashMap, sync::Arc, time::Duration};

use http::Extensions;
use reqwest::{
    header::{HeaderMap, RETRY_AFTER},
    Method, Request, Response, ResponseBuilderExt, StatusCode, Url, Version,
};
use reqwest_middleware::{Error, Middleware, Next};
use tokio::{
    sync::{watch, Mutex},
    time::Instant,
};

const GET_DEDUPLICATION_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
struct CachedResponse {
    status: StatusCode,
    version: Version,
    headers: HeaderMap,
    body: Arc<[u8]>,
    url: Url,
}

impl CachedResponse {
    async fn from_response(response: Response) -> Result<Self, reqwest::Error> {
        let status = response.status();
        let version = response.version();
        let headers = response.headers().clone();
        let url = response.url().clone();
        let body = Arc::from(response.bytes().await?.as_ref());

        Ok(Self {
            status,
            version,
            headers,
            body,
            url,
        })
    }

    fn to_response(&self) -> Response {
        let mut response = http::Response::builder()
            .status(self.status)
            .version(self.version)
            .url(self.url.clone())
            .body(self.body.to_vec())
            .expect("cached Spotify API response is valid");
        *response.headers_mut() = self.headers.clone();
        response.into()
    }
}

#[derive(Clone, Debug)]
enum SharedGetResult {
    Response(CachedResponse),
    Error(String),
}

#[derive(Debug)]
struct RecentGet {
    completed_at: Arc<parking_lot::Mutex<Option<Instant>>>,
    result: watch::Receiver<Option<Arc<SharedGetResult>>>,
}

#[derive(Debug, Default)]
struct RequestState {
    retry_after_until: Option<Instant>,
    recent_gets: HashMap<String, RecentGet>,
}

#[derive(Clone, Debug)]
struct SpotifyApiRequestManager {
    state: Arc<Mutex<RequestState>>,
    max_retries: usize,
}

struct GetLeader {
    completed_at: Arc<parking_lot::Mutex<Option<Instant>>>,
    result: watch::Sender<Option<Arc<SharedGetResult>>>,
}

impl GetLeader {
    fn publish(&self, result: SharedGetResult) {
        *self.completed_at.lock() = Some(Instant::now());
        self.result.send_replace(Some(Arc::new(result)));
    }
}

enum GetRegistration {
    Leader(GetLeader),
    Shared(watch::Receiver<Option<Arc<SharedGetResult>>>),
}

impl SpotifyApiRequestManager {
    fn new(max_retries: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(RequestState::default())),
            max_retries,
        }
    }

    async fn register_get(&self, request_key: String) -> GetRegistration {
        let now = Instant::now();
        let mut state = self.state.lock().await;
        state
            .recent_gets
            .retain(|_, request| match *request.completed_at.lock() {
                Some(completed_at) => {
                    now.saturating_duration_since(completed_at) < GET_DEDUPLICATION_WINDOW
                }
                // Remove dropped requests (closed watch channel)
                None => request.result.has_changed().is_ok(),
            });

        if let Some(request) = state.recent_gets.get(&request_key) {
            return GetRegistration::Shared(request.result.clone());
        }

        let (result_sender, result_receiver) = watch::channel(None);
        let completed_at = Arc::new(parking_lot::Mutex::new(None));
        state.recent_gets.insert(
            request_key,
            RecentGet {
                completed_at: Arc::clone(&completed_at),
                result: result_receiver,
            },
        );
        GetRegistration::Leader(GetLeader {
            completed_at,
            result: result_sender,
        })
    }

    async fn wait_for_retry_after(&self) {
        loop {
            let retry_after_until = {
                let mut state = self.state.lock().await;
                match state.retry_after_until {
                    Some(deadline) if deadline > Instant::now() => Some(deadline),
                    Some(_) => {
                        state.retry_after_until = None;
                        None
                    }
                    None => None,
                }
            };

            let Some(retry_after_until) = retry_after_until else {
                return;
            };

            tracing::debug!(
                wait_ms = retry_after_until
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
                "Rescheduling Spotify Web API GET request after global rate limit"
            );
            tokio::time::sleep_until(retry_after_until).await;
        }
    }

    async fn store_retry_after(&self, retry_after: Duration) {
        let retry_after_until = Instant::now() + retry_after + Duration::from_secs(1);
        let mut state = self.state.lock().await;
        if state
            .retry_after_until
            .is_none_or(|current| current < retry_after_until)
        {
            state.retry_after_until = Some(retry_after_until);
        }
    }

    async fn shared_result(
        mut result: watch::Receiver<Option<Arc<SharedGetResult>>>,
    ) -> reqwest_middleware::Result<Response> {
        loop {
            if let Some(result) = result.borrow().clone() {
                return match result.as_ref() {
                    SharedGetResult::Response(response) => Ok(response.to_response()),
                    SharedGetResult::Error(error) => {
                        Err(Error::Middleware(anyhow::anyhow!(error.clone())))
                    }
                };
            }

            result.changed().await.map_err(|_| {
                Error::Middleware(anyhow::anyhow!(
                    "shared Spotify Web API GET request was cancelled"
                ))
            })?;
        }
    }

    #[cfg(test)]
    async fn retry_after_remaining(&self) -> Option<Duration> {
        self.state
            .lock()
            .await
            .retry_after_until
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }
}

#[derive(Clone, Debug)]
pub(super) struct SpotifyApiMiddleware {
    api_base_url: Url,
    requests: SpotifyApiRequestManager,
}

impl SpotifyApiMiddleware {
    pub(super) fn new(api_base_url: &str, max_retries: usize) -> anyhow::Result<Self> {
        Ok(Self {
            api_base_url: Url::parse(api_base_url)?,
            requests: SpotifyApiRequestManager::new(max_retries),
        })
    }

    fn is_api_request(&self, url: &Url) -> bool {
        let base_path = self.api_base_url.path().trim_end_matches('/');
        let request_path = url.path();

        url.scheme() == self.api_base_url.scheme()
            && url.host_str() == self.api_base_url.host_str()
            && url.port_or_known_default() == self.api_base_url.port_or_known_default()
            && (request_path == base_path
                || request_path
                    .strip_prefix(base_path)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    }

    fn retry_after(headers: &HeaderMap) -> Option<Duration> {
        headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
    }

    async fn observe_rate_limit(&self, response: &Response, method: &Method, url: &Url) {
        if response.status() != StatusCode::TOO_MANY_REQUESTS {
            return;
        }

        if let Some(retry_after) = Self::retry_after(response.headers()) {
            self.requests.store_retry_after(retry_after).await;
            tracing::warn!(
                %method,
                %url,
                retry_after_secs = retry_after.as_secs(),
                "Spotify Web API rate limit encountered"
            );
        } else {
            tracing::warn!(
                %method,
                %url,
                "Spotify Web API rate limit encountered without a valid Retry-After duration"
            );
        }
    }

    async fn run_get(
        &self,
        request: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let url = request.url();
        match self.requests.register_get(url.to_string()).await {
            GetRegistration::Shared(result) => {
                tracing::info!(
                    %url,
                    "Sharing recent Spotify Web API GET response"
                );
                SpotifyApiRequestManager::shared_result(result).await
            }
            GetRegistration::Leader(leader) => {
                tracing::info!(
                    %url,
                    "Making a Spotify Web API GET request"
                );
                let response = self.run_get_with_retries(request, extensions, next).await;
                match response {
                    Ok(response) => match CachedResponse::from_response(response).await {
                        Ok(response) => {
                            leader.publish(SharedGetResult::Response(response.clone()));
                            Ok(response.to_response())
                        }
                        Err(error) => {
                            leader.publish(SharedGetResult::Error(error.to_string()));
                            Err(error.into())
                        }
                    },
                    Err(error) => {
                        leader.publish(SharedGetResult::Error(error.to_string()));
                        Err(error)
                    }
                }
            }
        }
    }

    async fn run_get_with_retries(
        &self,
        request: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let url = request.url();

        for attempt in 0..=self.requests.max_retries {
            self.requests.wait_for_retry_after().await;
            if attempt > 0 {
                tracing::warn!(
                    %url,
                    retry = attempt,
                    max_retries = self.requests.max_retries,
                    "Retrying rate-limited Spotify Web API GET request"
                );
            }
            let Some(request) = request.try_clone() else {
                return Err(Error::Middleware(anyhow::anyhow!(
                    "Spotify Web API GET request cannot be cloned for retry"
                )));
            };

            let response = next.clone().run(request, extensions).await?;
            self.observe_rate_limit(&response, &Method::GET, url).await;

            let retry_after = Self::retry_after(response.headers());
            if response.status() != StatusCode::TOO_MANY_REQUESTS
                || retry_after.is_none()
                || attempt == self.requests.max_retries
            {
                return Ok(response);
            }
        }

        unreachable!("GET retry loop always returns a response or error")
    }
}

#[async_trait::async_trait]
impl Middleware for SpotifyApiMiddleware {
    async fn handle(
        &self,
        request: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        if !self.is_api_request(request.url()) {
            return next.run(request, extensions).await;
        }

        let method = request.method().to_owned();
        let url = request.url().to_owned();

        if method == Method::GET {
            return self.run_get(request, extensions, next).await;
        }

        tracing::info!(%method, %url, "Making a Spotify Web API request");

        let response = next.run(request, extensions).await;
        if let Ok(response) = &response {
            self.observe_rate_limit(response, &method, &url).await;
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{GetRegistration, SharedGetResult, SpotifyApiMiddleware, SpotifyApiRequestManager};
    use reqwest::{header::HeaderValue, StatusCode, Url};
    use reqwest_middleware::ClientBuilder;
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, Request as MockRequest, Respond, ResponseTemplate,
    };

    #[derive(Clone)]
    struct RateLimitThenOk {
        calls: Arc<AtomicUsize>,
    }

    impl Respond for RateLimitThenOk {
        fn respond(&self, _request: &MockRequest) -> ResponseTemplate {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(429).insert_header("retry-after", "0")
            } else {
                ResponseTemplate::new(200).set_body_string("ok")
            }
        }
    }

    fn middleware() -> SpotifyApiMiddleware {
        SpotifyApiMiddleware::new("https://api.spotify.com/v1", 2).unwrap()
    }

    #[test]
    fn limits_only_configured_api_base_url() {
        let middleware = middleware();

        assert!(middleware
            .is_api_request(&Url::parse("https://api.spotify.com/v1/artists/id").unwrap()));
        assert!(!middleware
            .is_api_request(&Url::parse("https://accounts.spotify.com/api/token").unwrap()));
        assert!(!middleware
            .is_api_request(&Url::parse("https://api.spotify.com/v10/artists/id").unwrap()));
    }

    #[test]
    fn parses_retry_after_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, HeaderValue::from_static("42"));

        assert_eq!(
            SpotifyApiMiddleware::retry_after(&headers),
            Some(Duration::from_secs(42))
        );

        headers.insert(
            reqwest::header::RETRY_AFTER,
            HeaderValue::from_static("invalid"),
        );
        assert_eq!(SpotifyApiMiddleware::retry_after(&headers), None);
    }

    #[tokio::test(start_paused = true)]
    async fn stores_longest_global_retry_after() {
        let requests = SpotifyApiRequestManager::new(2);

        requests.store_retry_after(Duration::from_secs(10)).await;
        requests.store_retry_after(Duration::from_secs(5)).await;
        assert_eq!(
            requests.retry_after_remaining().await,
            Some(Duration::from_secs(11))
        );

        requests.store_retry_after(Duration::from_secs(20)).await;
        assert_eq!(
            requests.retry_after_remaining().await,
            Some(Duration::from_secs(21))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_global_retry_after() {
        let requests = SpotifyApiRequestManager::new(2);
        requests.store_retry_after(Duration::from_secs(10)).await;
        let waiting = tokio::spawn({
            let requests = requests.clone();
            async move { requests.wait_for_retry_after().await }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        tokio::time::advance(Duration::from_secs(11)).await;
        waiting.await.unwrap();
        assert_eq!(requests.retry_after_remaining().await, None);
    }

    #[tokio::test(start_paused = true)]
    async fn coalesces_identical_gets_for_one_second() {
        let requests = SpotifyApiRequestManager::new(2);
        let key = "https://api.spotify.com/v1/me".to_string();

        let GetRegistration::Leader(leader) = requests.register_get(key.clone()).await else {
            panic!("first GET should lead the shared request");
        };
        assert!(matches!(
            requests.register_get(key.clone()).await,
            GetRegistration::Shared(_)
        ));

        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(matches!(
            requests.register_get(key.clone()).await,
            GetRegistration::Shared(_)
        ));

        leader.publish(SharedGetResult::Error("done".to_string()));
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(matches!(
            requests.register_get(key).await,
            GetRegistration::Leader(_)
        ));
    }

    #[tokio::test]
    async fn retries_rate_limited_get() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/v1/me"))
            .respond_with(RateLimitThenOk {
                calls: Arc::clone(&calls),
            })
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(SpotifyApiMiddleware::new(&format!("{}/v1", server.uri()), 2).unwrap())
            .build();
        let response = client
            .get(format!("{}/v1/me", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn shares_identical_get_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/me"))
            .respond_with(ResponseTemplate::new(200).set_body_string("shared"))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(SpotifyApiMiddleware::new(&format!("{}/v1", server.uri()), 2).unwrap())
            .build();
        let url = format!("{}/v1/me", server.uri());

        let first = client.get(&url).send().await.unwrap();
        let second = client.get(&url).send().await.unwrap();

        assert_eq!(first.text().await.unwrap(), "shared");
        assert_eq!(second.text().await.unwrap(), "shared");
    }

    #[tokio::test]
    async fn does_not_retry_mutation() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/playlists"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "30"))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(SpotifyApiMiddleware::new(&format!("{}/v1", server.uri()), 2).unwrap())
            .build();
        let response = client
            .post(format!("{}/v1/playlists", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
