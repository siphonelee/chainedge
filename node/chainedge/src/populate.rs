use crate::{
    WrappedError,
    decode_cache_key,
    PROXY_ORIGIN_DOMAIN, PROXY_FROM_DOMAIN,
    InnerCachedResponse,
    http_response_from_parts,
    CachedResponse,
    IntoInnerCachedRequest, IntoInnerCachedResponse,
    CACHE_DIR,
};

use http::{header::HOST, Method, Request, Uri};
use http_cache_semantics::CachePolicy;
use std::time::SystemTime;
use miette::{miette, Context, IntoDiagnostic};


pub(crate) async fn populate(cache_key: String) -> Result<(), WrappedError> {
    let (mut method, url) = decode_cache_key(&cache_key);
    method = method.to_uppercase();
    let cache_key = crate::cache_key(method.clone(), url.clone());

    let path = url
        .parse::<Uri>()
        .into_diagnostic()?
        .path_and_query()
        .cloned()
        .unwrap();

    let proxy_url = http::Uri::builder()
        .scheme("http")
        .authority(PROXY_ORIGIN_DOMAIN)
        .path_and_query(path.clone())
        .build()
        .into_diagnostic()?;

    let client = reqwest::Client::new();
    let method: Method = method.parse().map_err(|_| miette::miette!("Method parse failed"))?;

    let origin_response = client
        .request(method.clone(), proxy_url.to_string())
        .send()
        .await
        .into_diagnostic()?;

    let origin_status = origin_response.status();
    let origin_headers = origin_response.headers().clone();
    let origin_version = origin_response.version();
    let origin_bytes = origin_response
        .bytes()
        .await
        .into_diagnostic()?;

    let parts = InnerCachedResponse {
        status_code: origin_status,
        headers: origin_headers.clone(),
        body: origin_bytes.into(),
        version: origin_version,
    };
    let response_to_cache = http_response_from_parts(parts.clone())
        .map_err(|_| miette::miette!("Could not build response"))?;
    let request_to_cache: Request<()> = Request::builder()
        .method(method)
        .uri(path)
        .header(HOST, PROXY_FROM_DOMAIN)
        .body(())
        .into_diagnostic()?;

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

    Ok(())
}

pub(crate) async fn remove(cache_key: String) -> Result<(), WrappedError> {
    let (mut method, url) = decode_cache_key(&cache_key);
    method = method.to_uppercase();
    let cache_key = crate::cache_key(method.clone(), url.clone());

    return cacache::remove(CACHE_DIR, cache_key).await
            .map_err(|_| miette!("record thread error").into());
    //        .into_diagnostic()?
}