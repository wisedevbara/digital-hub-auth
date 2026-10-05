use actix_cors::Cors;
use actix_files::Files;
use actix_web::{middleware::Logger, web, App, HttpRequest, HttpResponse, HttpServer};
use serde::Serialize;
use std::collections::HashMap;
use std::env;

/// Proxy the Authentik flow executor so the SPA can render the challenges
/// itself instead of showing Authentik's built-in pages.
///
/// The Authentik session cookie must be preserved between calls or the flow
/// context (pending_user, consent token, `next`) is lost, so cookies are
/// echoed straight through to the browser.
async fn flow_executor(req: HttpRequest, body: web::Bytes) -> HttpResponse {
    // A trailing slash must not change the matched slug, otherwise
    // "dh-login/" silently misses the allow-list and 404s before the guard.
    let flow_slug = req
        .match_info()
        .query("flow_slug")
        .trim_end_matches('/')
        .to_ascii_lowercase();

    log::info!("flow_executor slug={flow_slug:?} raw_path={}", req.path());

    // Flows exposed to the custom UI. The list is configuration, not code, so a
    // second provider can use its own flow (dh-login-mfa, dh-consent-strict, ...)
    // without a rebuild. Defaults keep the single-provider setup working.
    if !allowed_flows().iter().any(|f| f == &flow_slug) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "detail": format!("flow '{flow_slug}' is not exposed to the custom UI"),
        }));
    }

    let query = req
        .query_string()
        .split_once("query=")
        .map(|(_, q)| q.to_string())
        .unwrap_or_default();

    let mut url = format!(
        "{}/api/v3/flows/executor/{}/",
        authentik_base(),
        flow_slug
    );
    if !query.is_empty() {
        url.push_str("?query=");
        url.push_str(&query);
    }

    let client = match reqwest::Client::builder()
        // Authentik answers a stage transition with a 302 to the same URL;
        // following it transparently here is what keeps the SPA simple.
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    let mut builder = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_vec());

    // Django's CSRF check compares Origin against the request Host. reqwest
    // would send Host: server:9000, which never matches the browser-facing
    // origin, so pin Host to the public host and rewrite Origin/Referer to
    // match it. The CSRF token itself is relayed from the authentik_csrf
    // cookie in the X-Authentik-CSRF header that authentik expects.
    //
    // Note the Host header must be host[:port] only - including the scheme
    // makes the request unparseable and authentik answers 404.
    let idp_origin = authentik_origin();
    let idp_host = idp_origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&idp_origin)
        .to_string();
    builder = builder.header(reqwest::header::HOST, idp_host);
    builder = builder.header(reqwest::header::ORIGIN, idp_origin.clone());
    builder = builder.header(reqwest::header::REFERER, format!("{idp_origin}/"));

    if let Some(cookie) = req.headers().get("cookie").and_then(|c| c.to_str().ok()) {
        builder = builder.header(reqwest::header::COOKIE, cookie);

        if let Some(csrf) = read_cookie(cookie, "authentik_csrf") {
            builder = builder.header("X-Authentik-CSRF", csrf);
        }
    }

    let response = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("flow executor request failed: {e}");
            return HttpResponse::BadGateway()
                .json(serde_json::json!({ "detail": format!("identity provider unreachable: {e}") }));
        }
    };

    // reqwest and actix pull in different `http` crate versions, so the
    // status must be converted explicitly rather than passed through.
    let status = actix_web::http::StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(actix_web::http::StatusCode::BAD_GATEWAY);
    let mut out = HttpResponse::build(status);

    // Relay set-cookie so the Authentik session sticks across SPA calls.
    // Manual construction is used because the upstream value already contains
    // a full attribute string and Cookie::parse_encoded is strict about it.
    for value in response.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Ok(v) = value.to_str() {
            let raw = v.to_owned();
            let name_value = raw.split(';').next().unwrap_or("").trim().to_string();

            if let Some((name, val)) = name_value.split_once('=') {
                let name = name.trim().to_string();
                let val = val.trim().to_string();

                if !name.is_empty() {
                    // Path "/" not "/idp": the browser also calls the Laravel
                    // app on this origin, and a narrower path would keep the
                    // session cookie from being sent there.
                    out.cookie(
                        actix_web::cookie::Cookie::build(name, val)
                            .path("/")
                            .http_only(true)
                            .secure(false)
                            .same_site(actix_web::cookie::SameSite::Lax)
                            .finish(),
                    );
                }
            }
        }
    }

    match response.bytes().await {
        Ok(bytes) => out.content_type("application/json").body(bytes),
        Err(e) => HttpResponse::BadGateway().json(serde_json::json!({ "detail": e.to_string() })),
    }
}

/// Report where Authentik's authorize endpoint redirects, so the SPA knows
/// which flow to drive (dh-login vs dh-consent) without a page navigation.
async fn authorize_entry(req: HttpRequest) -> HttpResponse {
    let qs = req.query_string().to_string();
    let url = format!("{}/application/o/authorize/?{}", authentik_base(), qs);

    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    let mut builder = client.get(&url);
    // Must forward the Authentik session cookie, otherwise the IdP always
    // answers "not logged in" and re-runs the login flow.
    if let Some(cookie) = req.headers().get("cookie").and_then(|c| c.to_str().ok()) {
        builder = builder.header(reqwest::header::COOKIE, cookie);
    }

    match builder.send().await {
        Ok(resp) if resp.status().is_redirection() => {
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            HttpResponse::Ok().json(serde_json::json!({
                "location": location,
                "status": resp.status().as_u16(),
            }))
        }
        Ok(resp) => HttpResponse::Ok().json(serde_json::json!({
            "location": null,
            "status": resp.status().as_u16(),
        })),
        Err(e) => HttpResponse::BadGateway()
            .json(serde_json::json!({ "detail": format!("identity provider unreachable: {e}") })),
    }
}

fn authentik_base() -> String {
    env::var("AUTHENTIK_URL").unwrap_or_else(|_| "http://server:9000".to_string())
}

/// Flow slugs the SPA is allowed to drive, lower-cased.
///
/// Configuration, not code: a second provider may point at a different flow
/// (`dh-login-mfa`, `dh-consent-strict`, ...) and adding it must not require a
/// rebuild. Empty entries are dropped so a trailing comma in the env var is
/// harmless.
fn allowed_flows() -> Vec<String> {
    static CACHE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let raw = env::var("ALLOWED_FLOWS").unwrap_or_else(|_| "dh-login,dh-consent".into());
            raw.split(',')
                .map(|s| s.trim().trim_end_matches('/').to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .clone()
}

/// Login flow used when the authorize redirect names none.
fn default_login_flow() -> String {
    env::var("DEFAULT_LOGIN_FLOW").unwrap_or_else(|_| "dh-login".into())
}

/// Consent flow used when the authorize redirect names none.
fn default_consent_flow() -> String {
    env::var("DEFAULT_CONSENT_FLOW").unwrap_or_else(|_| "dh-consent".into())
}

/// Generic page to send the browser to when there is no authorize request to
/// complete. Configurable because with several client apps there is no single
/// "home" page anymore - this is only the last-resort recovery target.
fn start_url() -> String {
    env::var("IDP_START_URL").unwrap_or_else(|_| "/".into())
}

/// Per-client overrides, keyed by `client_id`.
///
/// One JSON env var so the registry can grow without a rebuild, e.g.
///   IDP_CLIENTS='{"abc":{"name":"Next Portal","start_url":"http://localhost:3000/login"}}'
/// Unknown clients fall back to the global defaults, so registering a client is
/// optional - only needed when it wants its own branding or recovery page.
fn client_overrides() -> serde_json::Value {
    static CACHE: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| match env::var("IDP_CLIENTS") {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                log::warn!("IDP_CLIENTS is not valid JSON, ignoring: {e}");
                serde_json::json!({})
            }),
            Err(_) => serde_json::json!({}),
        })
        .clone()
}

/// One client's overrides, always an object (empty when unregistered).
fn client_settings(client_id: &str) -> serde_json::Value {
    client_overrides()
        .get(client_id)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Pull one cookie's value out of a raw `Cookie:` header.
fn read_cookie(header: &str, name: &str) -> Option<String> {
    for part in header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

/// Scheme://host[:port] of the IdP as the browser sees it, used for the
/// Origin/Referer headers Django's CSRF check expects.
fn authentik_origin() -> String {
    env::var("AUTHENTIK_PUBLIC_URL")
        .unwrap_or_else(|_| "http://localhost:9001".into())
        .trim_end_matches('/')
        .to_string()
}

/// SPA fallback: rewrite unknown routes to index.html so Vue Router works.
async fn spa_fallback() -> HttpResponse {
    match std::fs::read("./dist/index.html") {
        Ok(body) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(body),
        Err(_) => HttpResponse::NotFound().body("index.html not found"),
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(Health {
        status: "ok",
        service: "digital-hub-auth-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Everything the SPA needs to render the right screens, resolved for one
/// client.
///
/// `client_id` is optional: the SPA passes it when it has one, and without it
/// the global defaults come back. Per-client values override the global ones, so
/// one IdP can front many apps with different branding and recovery pages.
async fn config(req: HttpRequest) -> HttpResponse {
    let client_id = web::Query::<HashMap<String, String>>::from_query(req.query_string())
        .ok()
        .and_then(|q| q.get("client_id").cloned())
        .unwrap_or_default();

    let overrides = client_settings(&client_id);

    // A per-client string wins; otherwise the global default stands.
    let pick = |key: &str, fallback: String| -> String {
        overrides
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or(fallback)
    };
    let text = |key: &str| -> String {
        overrides
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    HttpResponse::Ok().json(serde_json::json!({
        "client_id": client_id,
        // Browser-facing base URL for the IdP.
        "authentik_url": authentik_origin(),
        "login_flow": pick("login_flow", default_login_flow()),
        "consent_flow": pick("consent_flow", default_consent_flow()),
        // Where to send the browser when there is nothing to authorize.
        "start_url": pick("start_url", start_url()),
        // Branding; the shell falls back to its own defaults when absent.
        "brand_name": pick("brand_name", "Digital Hub".into()),
        "brand_subtitle": text("brand_subtitle"),
        // Human name of the requesting app, shown on the consent screen.
        "client_name": text("name"),
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = env::var("API_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    log::info!("starting digital-hub-auth-api on {host}:{port}");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .wrap(Cors::permissive())
            .route("/api/health", web::get().to(health))
            .route("/api/config", web::get().to(config))
            .route("/api/authorize/entry", web::get().to(authorize_entry))
            .route("/api/flow/{flow_slug}", web::post().to(flow_executor))
            // Static assets and the SPA shell are served from the root. The
            // earlier 404s on /api/flow were caused by a malformed Host
            // header sent upstream, not by this Files mount.
            .service(Files::new("/", "./dist").index_file("index.html"))
            .default_service(web::to(spa_fallback))
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}