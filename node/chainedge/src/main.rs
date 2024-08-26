use std::{fmt::Display, net::SocketAddr, time::SystemTime, error::Error, time::Duration};

use axum::{
    body::{Body, Bytes},
    extract::{Host, State},
    response::IntoResponse,
    RequestExt, Router,
};

use http::{uri::PathAndQuery, HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use http_cache_semantics::{BeforeRequest, CachePolicy};
use maud::html;
use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use tower_cookies::CookieManagerLayer;
use tracing::info;

use ethers::prelude::*;
use ethers::providers::{Provider, Http};
use ethers_core::types::Address;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::atomic::Ordering; 
use tokio::time::sleep;
use tokio::signal;
use tower_http::timeout::TimeoutLayer;
use futures::StreamExt;

pub mod admin;
pub mod populate;

const PROXY_FROM_DOMAIN: &str = "slow.coreyja.com:3001";
const PROXY_ORIGIN_DOMAIN: &str = "slow-server.fly.dev:3000";
const CONTRACT_ADDRESS: &str = "0x365D9FFd3334d12f89f8510cd352b3DbB5f4Cf85";
const RPC_URL: &str = "https://rpc.open-campus-codex.gelato.digital/";

abigen!(IChainEdge, "./src/ChainEdge.json");

#[derive(Debug, Clone)]
struct AppState {
    admin_password: String,
    accumulated_cnt: Arc<AtomicU64>,
}

#[derive(Debug, Clone, EthEvent)]
pub struct NewLink {
    pub link: String,
}

#[derive(Debug, Clone, EthEvent)]
pub struct RemoveLink {
    pub link: String,
}

fn start_record_thread<T>(contract: Arc<IChainEdge<T>>, 
                            accumulate_cnt: Arc<AtomicU64>, 
                            stop_flag: Arc<AtomicBool>) -> Result<tokio::task::JoinHandle<()>, Box<dyn Error>> 
                            where T: ethers_middleware::Middleware+Sync+Send+'static {
    let jh = tokio::spawn(async move {
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let cnt = accumulate_cnt.swap(0u64, Ordering::SeqCst);
            if cnt > 0 {
                let _tx = contract.add_serve_count(U256::from(cnt)).send().await.unwrap().await.unwrap();
                // println!("Transaction Receipt: {}", serde_json::to_string(&_tx).into_diagnostic()?);
            
                let total = contract.get_serve_count().call().await.unwrap();
                // println!("Transaction Receipt: {}", serde_json::to_string(&tx).into_diagnostic()?);
            
                println!("added count: {}, total count: {}", cnt, total);
            } else {
                sleep(tokio::time::Duration::from_secs(1)).await;        
            }
        }
        println!("record task exit");
    });

    Ok(jh)    
}

async fn start_event_listening<T>(contract: Arc<IChainEdge<T>>) -> Result<tokio::task::JoinHandle<()>, Box<dyn Error>> 
                    where T: ethers_middleware::Middleware+Sync+Send+'static {
    let events = contract.events();
    
    let jh = tokio::spawn(async move {
        let mut stream = events.stream().await.unwrap();
        while let Some(Ok(evt)) = stream.next().await {
            match evt {
                IChainEdgeEvents::NewLinkFilter ( t ) => {
                    println!("Fetch link: {link}", link = t.link);
                    let _ = populate::populate(t.link).await.map_err(|e| println!("{}", e.0));
                },
                IChainEdgeEvents::RemoveLinkFilter(t ) => {
                    println!("Remove {link}", link = t.link);
                    let _ = populate::remove(t.link).await.map_err(|e| println!("{}", e.0));
                },
                _ => {}
            }
        }
        println!("event task exit");
    });

    Ok(jh)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let quit = async {
        signal::unix::signal(signal::unix::SignalKind::quit())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let quit = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { println!("CTRL-C detected"); },
        _ = terminate => { println!("SIGTERM detected"); },
        _ = quit => { println!("SIGQUIT detected"); },
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let admin_password = std::env::var("ADMIN_AUTH_KEY").into_diagnostic()?;

    let contract_address = CONTRACT_ADDRESS.parse::<Address>().into_diagnostic()?;

    // contract info
    let rpc_url = RPC_URL.to_owned();
    let provider = Provider::<Http>::try_from(rpc_url.as_str()).into_diagnostic()?;

    let chain_id = provider.get_chainid().await.into_diagnostic()?;

    let wallet = std::env::var("WALLET_PRIV_KEY")
                    .into_diagnostic()?
                    .parse::<LocalWallet>()
                    .into_diagnostic()?
                    .with_chain_id(chain_id.as_u64());
    let client = SignerMiddleware::new(provider, wallet);

    let provider = Arc::new(client);
    let contract = Arc::new(IChainEdge::new(contract_address, provider.clone()));
    
    let stop_flag = Arc::new(AtomicBool::new(false));
    let accumulated_cnt = Arc::new(AtomicU64::new(0));

    let app_state = AppState {
        admin_password,
        accumulated_cnt: accumulated_cnt.clone(),
    };

    let record_jh = start_record_thread(contract.clone(), accumulated_cnt, stop_flag.clone())
                        .map_err(|_| miette!("record thread error"))?;
    let event_jh = start_event_listening(contract.clone()).await.map_err(|_| miette!("event thread error"))?;

    let app = Router::new()
        .route("/_chainedge/auth", axum::routing::get(admin::auth::get))
        .route("/_chainedge/auth", axum::routing::post(admin::auth::post))
        .route("/_chainedge/list", axum::routing::get(admin::list::route))
        .route(
            "/_chainedge/clear_fs",
            axum::routing::post(admin::clear_fs::route),
        )
        .fallback(proxy_request)
        .layer((CookieManagerLayer::new(), TimeoutLayer::new(Duration::from_secs(6)),))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .into_diagnostic()?;

    stop_flag.store(true, Ordering::Relaxed);
    record_jh.await.into_diagnostic()?;
    event_jh.abort();

    Ok(())
}

// #[axum_macros::debug_handler]
async fn proxy_request(
    State(app_state): State<AppState>,
    mut request: Request<Body>,
) -> Result<impl IntoResponse, String> {
    let host: Host = request
        .extract_parts()
        .await
        .map_err(|_| "Could not extract host")?;

    if host.0 != PROXY_FROM_DOMAIN {
        return Err(format!(
            "We only proxy requests to the specified domain. Found: {} Expected: {}",
            host.0, PROXY_FROM_DOMAIN
        ));
    }

    let response = get_potentially_cached_response(request, app_state)
        .await
        .map_err(|e| e.to_string())?;

    Ok((
        response.status(),
        response.headers().clone(),
        response.into_body(),
    ))
}

const CACHE_DIR: &str = "./tmp/cache";

#[derive(Deserialize, Serialize)]
struct InnerCachedRequest {
    #[serde(with = "http_serde::method")]
    pub method: Method,

    #[serde(with = "http_serde::uri")]
    pub uri: Uri,

    #[serde(with = "http_serde::version")]
    pub version: Version,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,

    // TODO: Can this just be a Bytes
    body: Option<Vec<u8>>,
}

#[derive(Deserialize, Serialize, Clone)]
struct InnerCachedResponse {
    #[serde(with = "http_serde::status_code")]
    pub status_code: StatusCode,

    #[serde(with = "http_serde::version")]
    pub version: Version,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,

    body: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
struct CachedResponse {
    request: InnerCachedRequest,
    response: InnerCachedResponse,
    cached_at: SystemTime,
}

async fn get_policy_from_cache(key: &str) -> Result<(CachePolicy, http::Response<Bytes>)> {
    let cached = cacache::read(CACHE_DIR, key)
        .await
        .context("Could not read from cache")?;
    let cached = postcard::from_bytes::<CachedResponse>(&cached)
        .map_err(|_| miette!("Could not deserialize cached response"))?;

    let response = http_response_from_parts(cached.response)
        .map_err(|_| miette!("Could not build response"))?;

    let request =
        http_request_from_parts(cached.request).map_err(|_| miette!("Could not build request"))?;

    let policy =
        CachePolicy::new_options(&request, &response, cached.cached_at, Default::default());

    Ok((policy, response))
}

pub fn cache_key(method: impl Display, url: impl Display) -> String {
    format!("{}\t{}", method, url)
}

pub fn decode_cache_key(cache_key: &String) -> (String, String) {
    let parts: Vec<&str> = cache_key.split("\t").collect();
    if parts.len() != 2 {
        return (String::new(), String::new());
    }

    (parts[0].to_owned(), parts[1].to_owned())
}

pub struct WrappedError(miette::Report);

impl IntoResponse for WrappedError {
    fn into_response(self) -> axum::response::Response {
        let err = self.0.to_string();
        let resp = html! {
            h1 { "Error" }
            p { (err) }
        };

        (StatusCode::INTERNAL_SERVER_ERROR, resp).into_response()
    }
}

impl<E> From<E> for WrappedError
where
    E: Into<miette::Report>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[tracing::instrument(skip_all)]
async fn get_potentially_cached_response(
    request: Request<Body>,
    app_state: AppState,
) -> Result<http::Response<Bytes>> {

    let method = request.method().clone();
    let url = request.uri().clone();
    info!("Requesting: {}", url);
    let cache_key = cache_key(&method, &url);

    {
        let policy = get_policy_from_cache(&cache_key).await;

        if let Ok((policy, response)) = policy {
            let can_cache = policy.before_request(&request, SystemTime::now());

            match can_cache {
                // TODO: Use the Parts from Fresh to build the response
                BeforeRequest::Fresh(parts) => {
                    info!(parts =? parts, "Cache hit for: {}", url);
                    let resp_size = response.body().len();
                    app_state.accumulated_cnt.fetch_add(resp_size as u64, Ordering::SeqCst);
                    return Ok(response);
                }
                BeforeRequest::Stale {
                    matches,
                    request: revalidation_request,
                } => {
                    info!(
                        matches =? matches,
                        revalidation_request =? revalidation_request,
                        original_request =? request,
                        ttl =? policy.time_to_live(SystemTime::now()),
                        "Cache hit for: {} but not-usable", url
                    );
                }
            };
        }
    }

    let path = url
        .path_and_query()
        .cloned()
        .unwrap_or_else(|| PathAndQuery::from_static("/"));

    let proxy_url = http::Uri::builder()
        .scheme("http")
        .authority(PROXY_ORIGIN_DOMAIN)
        .path_and_query(path.clone())
        .build()
        .map_err(|_| miette!("Could not build url"))?;

    let headers = request.headers().clone();
    let bytes = hyper::body::to_bytes(request.into_body())
        .await
        .map_err(|_| miette!("Could not get bytes from body"))?;
    let client = reqwest::Client::new();
    let origin_response = client
        .request(method.clone(), proxy_url.to_string())
        .headers(headers.clone())
        .body(bytes.clone())
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .map_err(|_| miette!("Request failed"))?;

    let origin_status = origin_response.status();
    let origin_headers = origin_response.headers().clone();
    let origin_version = origin_response.version();
    let origin_bytes = origin_response
        .bytes()
        .await
        .map_err(|_| miette!("Could not get bytes from body"))?;

    let parts = InnerCachedResponse {
        status_code: origin_status,
        headers: origin_headers.clone(),
        body: origin_bytes.into(),
        version: origin_version,
    };
    let response_to_cache =
        http_response_from_parts(parts.clone()).map_err(|_| miette!("Could not build response"))?;
    let mut request_to_cache = Request::builder().method(method.clone()).uri(url.clone());
    for (key, value) in headers {
        if let Some(key) = key {
            request_to_cache = request_to_cache.header(key, value);
        }
    }

    let request_to_cache = request_to_cache
        .body(bytes)
        .map_err(|_| miette!("Could not build request"))?;

    let policy = CachePolicy::new(&request_to_cache, &response_to_cache);
    if policy.is_storable() && !policy.time_to_live(SystemTime::now()).is_zero() {
        let response_to_cache = CachedResponse {
            request: request_to_cache.into_inner_cached_request()?,
            response: response_to_cache.into_inner_cached_response()?,
            cached_at: SystemTime::now(),
        };

        cacache::write(
            CACHE_DIR,
            cache_key,
            postcard::to_allocvec(&response_to_cache).into_diagnostic()?,
        )
        .await
        .context("Could not write to cache")?;
    }

    let response =
        http_response_from_parts(parts).map_err(|_| miette::miette!("Could not build response"))?;

    Ok(response)
}

fn http_response_from_parts(parts: InnerCachedResponse) -> Result<http::Response<Bytes>> {
    let InnerCachedResponse {
        status_code,
        headers,
        body,
        version,
    } = parts;

    let mut builder = http::Response::builder()
        .status(status_code)
        .version(version);

    for (key, value) in headers.iter() {
        builder = builder.header(key, value);
    }

    let body: Bytes = body.into();

    builder.body(body).into_diagnostic()
}

fn http_request_from_parts(parts: InnerCachedRequest) -> Result<http::Request<Bytes>> {
    let InnerCachedRequest {
        method,
        uri,
        version,
        headers,
        body,
    } = parts;

    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);

    for (key, value) in headers.iter() {
        builder = builder.header(key, value);
    }

    let body: Bytes = if let Some(b) = body {
        b.into()
    } else {
        Bytes::new()
    };

    builder.body(body).into_diagnostic()
}

trait IntoInnerCachedRequest {
    fn into_inner_cached_request(self) -> Result<InnerCachedRequest>;
}

impl IntoInnerCachedRequest for Request<Bytes> {
    fn into_inner_cached_request(self) -> Result<InnerCachedRequest> {
        let (parts, body) = self.into_parts();

        Ok(InnerCachedRequest {
            method: parts.method,
            uri: parts.uri,
            version: parts.version,
            headers: parts.headers,
            body: Some(body.to_vec()),
        })
    }
}

impl IntoInnerCachedRequest for Request<()> {
    fn into_inner_cached_request(self) -> Result<InnerCachedRequest> {
        let (parts, _) = self.into_parts();

        Ok(InnerCachedRequest {
            method: parts.method,
            uri: parts.uri,
            version: parts.version,
            headers: parts.headers,
            body: None,
        })
    }
}

trait IntoInnerCachedResponse {
    fn into_inner_cached_response(self) -> Result<InnerCachedResponse>;
}

impl IntoInnerCachedResponse for Response<Bytes> {
    fn into_inner_cached_response(self) -> Result<InnerCachedResponse> {
        let (parts, body) = self.into_parts();

        Ok(InnerCachedResponse {
            status_code: parts.status,
            version: parts.version,
            headers: parts.headers,
            body: body.to_vec(),
        })
    }
}
