use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};

use crate::routes::{backups, devices, media, photos, setup, storage, system, user};

use crate::auth::decode_token;
use crate::state::AppState;

async fn api_not_found(
    uri: axum::http::Uri,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({
            "ok": false,
            "error": "api_not_found",
            "status": 404,
            "path": uri.path(),
            "message": "İstenen PhotoOS API endpoint'i bulunamadı."
        })),
    )
}

// PHOTOOS FINAL RC SELECTIVE API AUTH GUARD V1
//
// Public setup/login/mobile/PC transfer protocols are deliberately not
// globally blocked here. Only management and observability API families
// are protected.
//
// GET/HEAD: authenticated PhotoOS user.
// Mutating requests: admin user.
//
// Browser WebSocket Authorization-header limitations require
// /api/v1/notifications/ws to remain on its existing contract for now.
// That endpoint will receive a token-query handshake in the dedicated
// notification WebSocket hardening pass.

fn photoos_public_media_read(method: &Method, path: &str) -> bool {
    if !matches!(*method, Method::GET | Method::HEAD) {
        return false;
    }

    path.starts_with("/api/v1/photos/thumb/")
        || path.starts_with("/api/v1/photos/file/")
}

fn photoos_protected_api(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "/api/v1/system",
        "/api/v1/storage",
        "/api/v1/raid",
        "/api/v1/backups",
        "/api/v1/pc-backups",
        "/api/v1/logs",
        "/api/v1/notifications",
        "/api/v1/dashboard",
        "/api/v1/devices",
        "/api/v1/users",
        "/api/v1/photos",
        "/api/v1/albums",
    ];

    if path == "/api/v1/pc-client/pairing"
        || path == "/api/v1/setup/preferences"
        || path == "/api/v1/me"
    {
        return true;
    }

    PREFIXES.iter().any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(*prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn photoos_admin_required(method: &Method, path: &str) -> bool {
    if path == "/api/v1/pc-client/pairing"
        || path == "/api/v1/devices"
        || path == "/api/v1/pc-backups"
        || path.starts_with("/api/v1/pc-backups/")
    {
        return true;
    }

    !matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS
    )
}

fn photoos_auth_error(
    status: StatusCode,
    error: &'static str,
    message: &'static str,
) -> Response {
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "error": error,
            "message": message
        })),
    )
        .into_response()
}

async fn photoos_api_auth_guard(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // CORS preflight must never require credentials here.
    if method == Method::OPTIONS {
        return next.run(request).await;
    }

    // Notification WebSocket will be hardened separately with
    // browser-compatible token-query authentication.
    if path == "/api/v1/notifications/ws" {
        return next.run(request).await;
    }

    if photoos_public_media_read(&method, &path) {
        return next.run(request).await;
    }

    if !photoos_protected_api(&path) {
        return next.run(request).await;
    }

    let Some(raw_authorization) =
        request.headers().get(header::AUTHORIZATION)
    else {
        return photoos_auth_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Bu PhotoOS API endpoint'i için oturum gereklidir.",
        );
    };

    let Ok(raw_authorization) = raw_authorization.to_str() else {
        return photoos_auth_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Authorization başlığı geçersiz.",
        );
    };

    let token = raw_authorization
        .strip_prefix("Bearer ")
        .unwrap_or(raw_authorization)
        .trim();

    if token.is_empty() {
        return photoos_auth_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Oturum anahtarı boş.",
        );
    }

    let claims = match decode_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return photoos_auth_error(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Oturum süresi dolmuş veya token geçersiz.",
            );
        }
    };

    let role_result: Result<Option<String>, sqlx::Error> =
        sqlx::query_scalar(
            "SELECT role FROM users WHERE id = ? LIMIT 1",
        )
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await;

    let role = match role_result {
        Ok(Some(role)) => role,

        Ok(None) => {
            return photoos_auth_error(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Oturuma ait kullanıcı bulunamadı.",
            );
        }

        Err(_) => {
            return photoos_auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth_database_error",
                "Oturum yetkisi doğrulanamadı.",
            );
        }
    };

    if photoos_admin_required(&method, &path)
        && role != "admin"
    {
        return photoos_auth_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Bu PhotoOS işlemi yalnızca yöneticiler tarafından yapılabilir.",
        );
    }

    next.run(request).await
}

pub fn create_router(state: AppState) -> Router {
    let auth_state = state.clone();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]);

    let frontend_dir = state.settings.frontend.dist_directory.clone();

    let frontend_index = state.settings.frontend.index_file.clone();

    println!("Frontend directory : {}", frontend_dir);
    println!("Frontend index     : {}", frontend_index);

    let frontend = ServeDir::new(frontend_dir).fallback(ServeFile::new(frontend_index));

    Router::new()
        .merge(crate::routes::health::router())
        .merge(system::router())
        .merge(user::router())
        .merge(setup::router())
        .merge(media::router())
        .merge(crate::routes::mobile::router())
        .merge(crate::routes::albums::routes())
        .merge(storage::router())
        .merge(crate::routes::raid::router())
        .merge(backups::router())
        .merge(crate::routes::notifications::router())
        .merge(crate::routes::logs::router())
        .merge(devices::router())
        .merge(photos::routes())
        .merge(crate::routes::dashboard::routes())
        .merge(crate::routes::updates::routes())
        .with_state(state)
        /*
         * Tanımsız API istekleri React SPA'ya düşmemeli.
         * /api ve /api/... için gerçek JSON 404 döndür.
         */
        .route("/api", axum::routing::any(api_not_found))
        .route("/api/{*path}", axum::routing::any(api_not_found))
        /*
         * API dışındaki bilinmeyen yollar React SPA
         * client-side routing için index.html'e düşebilir.
         */
        .fallback_service(frontend)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            auth_state,
            photoos_api_auth_guard,
        ))
        .layer(cors)
}


// PHOTOOS PHASE2A SECURITY POLICY TESTS
#[cfg(test)]
mod security_policy_tests {
    use super::*;

    #[test]
    fn sensitive_api_routes_require_auth() {
        for path in [
            "/api/v1/users",
            "/api/v1/photos",
            "/api/v1/photos/count",
            "/api/v1/photos/timeline",
            "/api/v1/photos/upload",
            "/api/v1/photos/download-zip",
            "/api/v1/albums",
            "/api/v1/albums/42",
            "/api/v1/me",
            "/api/v1/setup/preferences",
        ] {
            assert!(
                photoos_protected_api(path),
                "expected protected route: {path}"
            );
        }
    }

    #[test]
    fn media_get_compatibility_is_explicit() {
        assert!(photoos_public_media_read(
            &Method::GET,
            "/api/v1/photos/thumb/example.jpg"
        ));
        assert!(photoos_public_media_read(
            &Method::HEAD,
            "/api/v1/photos/file/example.jpg"
        ));
        assert!(!photoos_public_media_read(
            &Method::DELETE,
            "/api/v1/photos/file/example.jpg"
        ));
        assert!(!photoos_public_media_read(
            &Method::GET,
            "/api/v1/photos/count"
        ));
    }

    #[test]
    fn bootstrap_routes_remain_public() {
        for path in [
            "/api/v1/login",
            "/api/v1/setup/status",
            "/api/v1/setup/initialize",
        ] {
            assert!(
                !photoos_protected_api(path),
                "expected public bootstrap route: {path}"
            );
        }
    }
}


#[cfg(test)]
mod accelerated_admin_privacy_tests {
    use super::*;

    #[test]
    fn device_and_pc_library_metadata_is_admin_only() {
        assert!(photoos_admin_required(&Method::GET, "/api/v1/devices"));
        assert!(photoos_admin_required(&Method::GET, "/api/v1/pc-backups"));
        assert!(photoos_admin_required(&Method::GET, "/api/v1/pc-backups/browse"));
        assert!(!photoos_admin_required(&Method::GET, "/api/v1/photos"));
    }
}
