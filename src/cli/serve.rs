use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::core::model::Row;
use crate::store::file::Store;

// ── OpenAPI spec root ─────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        list_entries,
        add_entry,
        remove_hostname,
        remove_by_ip,
        edit_entry,
        disable_entry,
        enable_entry,
        toggle_entry,
    ),
    components(schemas(
        Row,
        AddBody,
        EditBody,
        AddResponse,
        CountResponse,
        MessageResponse,
        ErrorResponse,
    )),
    tags(
        (name = "entries", description = "Hosts entry management")
    ),
)]
struct ApiDoc;

// ── Shared state ──────────────────────────────────────────

struct AppState {
    hosts_file: PathBuf,
}

// ── Request / Response types ──────────────────────────────

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
struct ListQuery {
    /// Filter by IP address (exact match)
    ip: Option<String>,

    /// Filter by hostname (exact match)
    hostname: Option<String>,

    /// Filter pattern (supports * and ? wildcards)
    filter: Option<String>,
    /// Show only IPv4 entries
    ipv4: Option<bool>,
    /// Show only IPv6 entries
    ipv6: Option<bool>,

    /// Case insensitive match
    ignore_case: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
struct AddBody {
    /// IP address
    ip: String,
    /// Hostnames to add
    hosts: Vec<String>,
    /// Optional comment
    comment: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct EditBody {
    /// New IP address
    ip: String,
}

#[derive(Serialize, ToSchema)]
struct AddResponse {
    message: String,
    duplicates: Vec<String>,
}

#[derive(Serialize, ToSchema)]
struct CountResponse {
    count: usize,
    message: String,
}

#[derive(Serialize, ToSchema)]
struct MessageResponse {
    message: String,
}

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize, ToSchema, IntoParams)]
struct RemoveByIpQuery {
    /// IP address to remove all entries for
    ip: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────

/// List all hosts entries
///
/// Returns entries from the hosts file. By default rows with the same IP
/// are compacted into a single row. Supports filtering by pattern, IPv4/IPv6
/// only mode, and case-insensitive search.
#[utoipa::path(
    get,
    path = "/api/entries",
    tag = "entries",
    params(ListQuery),
    responses(
        (status = 200, description = "List of entries", body = Vec<Row>),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn list_entries(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let store = Store::new(&state.hosts_file);
    let entries = match store.load() {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let re = q
        .filter
        .as_ref()
        .and_then(|p| crate::cli::entry::build_regex(p, q.ignore_case.unwrap_or(false)).ok());

    let mut rows = Vec::new();
    for entry in &entries {
        if entry.disabled {
            continue;
        }
        if entry.canonical.is_empty() && entry.aliases.is_empty() {
            continue;
        }
        if let Some(ref ip) = q.ip {
            if entry.ip != *ip {
                continue;
            }
        }
        if let Some(ref hostname) = q.hostname {
            let matched =
                entry.canonical == *hostname || entry.aliases.iter().any(|a| a == hostname);
            if !matched {
                continue;
            }
        }
        if let Some(ref re) = re {
            let ok = re.is_match(&entry.ip)
                || entry.aliases.iter().any(|h| re.is_match(h))
                || re.is_match(&entry.canonical)
                || entry.comment.as_ref().is_some_and(|c| re.is_match(c));
            if !ok {
                continue;
            }
        }
        if q.ipv4.unwrap_or(false) && entry.ip.parse::<std::net::Ipv4Addr>().is_err() {
            continue;
        }
        if q.ipv6.unwrap_or(false) && entry.ip.parse::<std::net::Ipv6Addr>().is_err() {
            continue;
        }
        let host_str = if entry.aliases.is_empty() {
            entry.canonical.clone()
        } else {
            let mut parts = vec![entry.canonical.clone()];
            parts.extend(entry.aliases.clone());
            parts.join(" ")
        };
        rows.push(Row {
            ip: entry.ip.clone(),
            host: host_str,
            comment: entry.comment.clone(),
            canonical: Some(entry.canonical.clone()),
            aliases: entry.aliases.clone(),
        });
    }

    (StatusCode::OK, Json(serde_json::json!(rows))).into_response()
}

/// Add a new hosts entry
///
/// Adds one or more hostnames for an IP address. If the IP already exists
/// the hostnames are merged into the existing entry. Returns a list of
/// duplicate hostnames found on other IPs.
#[utoipa::path(
    post,
    path = "/api/entries",
    tag = "entries",
    request_body = AddBody,
    responses(
        (status = 201, description = "Entry added", body = AddResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn add_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddBody>,
) -> impl IntoResponse {
    // Validate inputs
    if !crate::util::validation::is_valid_ip(&body.ip) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid IP address: {}", body.ip)})),
        )
            .into_response();
    }
    for h in &body.hosts {
        if !crate::util::validation::is_valid_hostname(h) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("invalid hostname: {}", h)})),
            )
                .into_response();
        }
    }
    if let Some(ref c) = body.comment {
        if !crate::util::validation::is_valid_comment(c) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid comment: contains control characters or newlines"}))).into_response();
        }
    }
    let store = Store::new(&state.hosts_file);
    match store.add_entry(&body.ip, &body.hosts, body.comment.as_deref()) {
        Ok(duplicates) => {
            let msg = format!("Added {} {}", body.ip, body.hosts.join(" "));
            (
                StatusCode::CREATED,
                Json(AddResponse {
                    message: msg,
                    duplicates,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Remove an entry by hostname
///
/// Removes a specific hostname from the hosts file.
#[utoipa::path(
    delete,
    path = "/api/entries/{hostname}",
    tag = "entries",
    params(
        ("hostname" = String, Path, description = "Hostname to remove"),
    ),
    responses(
        (status = 200, description = "Hostname removed", body = CountResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn remove_hostname(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(hostname): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !crate::util::validation::is_valid_hostname(&hostname) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid hostname: {}", hostname)})),
        )
            .into_response();
    }
    let store = Store::new(&state.hosts_file);
    match store.remove_hostnames(std::slice::from_ref(&hostname)) {
        Ok(count) => (
            StatusCode::OK,
            Json(CountResponse {
                count,
                message: format!("Removed {} hostname(s)", count),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Remove all entries for an IP
///
/// Removes every entry matching the given IP address.
#[utoipa::path(
    delete,
    path = "/api/entries",
    tag = "entries",
    params(RemoveByIpQuery),
    responses(
        (status = 200, description = "Entries removed by IP", body = CountResponse),
        (status = 400, description = "Missing ip parameter", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn remove_by_ip(
    State(state): State<Arc<AppState>>,
    Query(q): Query<RemoveByIpQuery>,
) -> impl IntoResponse {
    let store = Store::new(&state.hosts_file);
    match q.ip.as_deref() {
        Some(ip) => match store.remove_by_ip(ip) {
            Ok(count) => (
                StatusCode::OK,
                Json(CountResponse {
                    count,
                    message: format!("Removed {} entry(s) for IP {}", count, ip),
                }),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        None => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing ip query parameter"})),
        )
            .into_response(),
    }
}

/// Move a hostname to a new IP
#[utoipa::path(
    put,
    path = "/api/entries/{hostname}",
    tag = "entries",
    params(
        ("hostname" = String, Path, description = "Hostname to move"),
    ),
    request_body = EditBody,
    responses(
        (status = 200, description = "Hostname moved", body = MessageResponse),
        (status = 404, description = "Hostname not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn edit_entry(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(hostname): axum::extract::Path<String>,
    Json(body): Json<EditBody>,
) -> impl IntoResponse {
    if !crate::util::validation::is_valid_hostname(&hostname) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid hostname: {}", hostname)})),
        )
            .into_response();
    }
    if !crate::util::validation::is_valid_ip(&body.ip) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid IP address: {}", body.ip)})),
        )
            .into_response();
    }
    let store = Store::new(&state.hosts_file);
    match store.move_hostname(&hostname, &body.ip) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Hostname '{}' not found", hostname)})),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::OK,
            Json(MessageResponse {
                message: format!("Moved '{}' to {}", hostname, body.ip),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Disable a hostname
///
/// Comments out the hostname entry in the hosts file.
#[utoipa::path(
    put,
    path = "/api/entries/{hostname}/disable",
    tag = "entries",
    params(
        ("hostname" = String, Path, description = "Hostname to disable"),
    ),
    responses(
        (status = 200, description = "Hostname disabled", body = MessageResponse),
        (status = 404, description = "Hostname not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn disable_entry(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(hostname): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !crate::util::validation::is_valid_hostname(&hostname) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid hostname: {}", hostname)})),
        )
            .into_response();
    }
    let store = Store::new(&state.hosts_file);
    match store.disable_hostname(std::slice::from_ref(&hostname)) {
        Ok(n) if n > 0 => (
            StatusCode::OK,
            Json(MessageResponse {
                message: format!("Disabled {}", hostname),
            }),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Hostname '{}' not found", hostname)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Enable a hostname
///
/// Uncomments the hostname entry in the hosts file.
#[utoipa::path(
    put,
    path = "/api/entries/{hostname}/enable",
    tag = "entries",
    params(
        ("hostname" = String, Path, description = "Hostname to enable"),
    ),
    responses(
        (status = 200, description = "Hostname enabled", body = MessageResponse),
        (status = 404, description = "Hostname not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn enable_entry(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(hostname): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !crate::util::validation::is_valid_hostname(&hostname) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid hostname: {}", hostname)})),
        )
            .into_response();
    }
    let store = Store::new(&state.hosts_file);
    match store.enable_hostname(std::slice::from_ref(&hostname)) {
        Ok(n) if n > 0 => (
            StatusCode::OK,
            Json(MessageResponse {
                message: format!("Enabled {}", hostname),
            }),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Hostname '{}' not found", hostname)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Toggle a hostname
///
/// Flips the hostname between enabled and disabled state.
#[utoipa::path(
    put,
    path = "/api/entries/{hostname}/toggle",
    tag = "entries",
    params(
        ("hostname" = String, Path, description = "Hostname to toggle"),
    ),
    responses(
        (status = 200, description = "Hostname toggled", body = MessageResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    )
)]
async fn toggle_entry(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(hostname): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !crate::util::validation::is_valid_hostname(&hostname) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid hostname: {}", hostname)})),
        )
            .into_response();
    }
    let store = Store::new(&state.hosts_file);
    match store.toggle_hostname(&hostname) {
        Ok(msg) => (StatusCode::OK, Json(MessageResponse { message: msg })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── Public entrypoint ─────────────────────────────────────

pub fn handle(cli: &crate::cli::Cli, port: u16, bind: &str, no_docs: bool) {
    let hosts_file = cli.hosts_file.clone();
    let state = Arc::new(AppState { hosts_file });

    let mut app = Router::new()
        .route("/api/entries", get(list_entries).post(add_entry))
        .route(
            "/api/entries/{hostname}",
            delete(remove_hostname).put(edit_entry),
        )
        .route("/api/entries/{hostname}/disable", put(disable_entry))
        .route("/api/entries/{hostname}/enable", put(enable_entry))
        .route("/api/entries/{hostname}/toggle", put(toggle_entry))
        .route("/api/entries", delete(remove_by_ip))
        // Limit concurrent requests to 32 workers
        .layer(tower_http::limit::RequestBodyLimitLayer::new(65536 * 1024))
        // Limit concurrent requests to 32 workers
        .layer(tower::limit::ConcurrencyLimitLayer::new(32))
        .layer(tower_http::cors::CorsLayer::new())
        .with_state(state);

    if !no_docs {
        app = app.merge(SwaggerUi::new("/docs").url("/api/openapi.json", ApiDoc::openapi()));
    }

    let addr: SocketAddr = match format!("{}:{}", bind, port).parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: invalid bind address '{}': {}", bind, e);
            std::process::exit(1);
        }
    };
    if !cli.quiet {
        eprintln!("Serving hosts API on http://{}", addr);
        eprintln!("API docs at http://{}/docs", addr);
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| {
                eprintln!("Error: failed to bind to {}: {}", addr, e);
                std::process::exit(1);
            });
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    });
}
