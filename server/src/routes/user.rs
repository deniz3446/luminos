use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::user::{
        create_user,
        list_users,
        login,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {

    Router::new()
        .route(
            "/api/v1/users",
            get(list_users)
                .post(create_user),
        )
        .route(
            "/api/v1/login",
            post(login),
        )
}
