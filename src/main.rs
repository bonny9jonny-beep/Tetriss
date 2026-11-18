//! FSociety Tetris — сервер на Axum.
//!
//! Что делает:
//!   GET  /            — HTML-оболочка, которая подгружает Rust→WASM клиент
//!   GET  /pkg/*       — собранный WASM-бандл (папка PKG_DIR)
//!   GET  /health      — проверка живости
//!   GET  /api/scores  — топ рекордов из Postgres (?limit=1..50, по умолчанию 10)
//!   POST /api/scores  — сохранить рекорд {name, score, lines, level}

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    FromRow, PgPool,
};
use std::{env, net::SocketAddr, str::FromStr, time::Duration};
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
};

const SHELL_HTML: &str = r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover">
<title>Tetris FSociety</title>
</head>
<body>
<script type="module">
  import init from '/pkg/fsociety_tetris_wasm.js';
  init();
</script>
</body>
</html>"#;

// Верхние границы, чтобы в таблицу не попал мусор.
const MAX_NAME_CHARS: usize = 16;
const MAX_SCORE: i32 = 5_000_000;
const MAX_LINES: i32 = 20_000;
const MAX_LEVEL: i32 = 2_000;

#[derive(Clone)]
struct AppState {
    /// None — если DATABASE_URL не задан: игра работает, таблица рекордов отключена.
    db: Option<PgPool>,
}

#[derive(Serialize, FromRow)]
struct ScoreRow {
    name: String,
    score: i32,
    lines: i32,
    level: i32,
}

#[derive(Deserialize)]
struct NewScore {
    name: String,
    score: i32,
    lines: i32,
    level: i32,
}

#[derive(Serialize)]
struct Saved {
    rank: i64,
}

#[derive(Deserialize)]
struct ListParams {
    limit: Option<i64>,
}

enum ApiError {
    NoDb,
    Bad(&'static str),
    Db(sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::NoDb => (StatusCode::SERVICE_UNAVAILABLE, "база данных не подключена").into_response(),
            ApiError::Bad(m) => (StatusCode::UNPROCESSABLE_ENTITY, m).into_response(),
            ApiError::Db(e) => {
                tracing::error!("db error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "ошибка базы данных").into_response()
            }
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e)
    }
}

async fn list_scores(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Vec<ScoreRow>>, ApiError> {
    let db = st.db.as_ref().ok_or(ApiError::NoDb)?;
    let limit = p.limit.unwrap_or(10).clamp(1, 50);
    let rows = sqlx::query_as::<_, ScoreRow>(
        "SELECT name, score, lines, level FROM scores ORDER BY score DESC, id ASC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(Json(rows))
}

async fn add_score(
    State(st): State<AppState>,
    Json(body): Json<NewScore>,
) -> Result<(StatusCode, Json<Saved>), ApiError> {
    let db = st.db.as_ref().ok_or(ApiError::NoDb)?;

    // Имя: без управляющих символов, обрезаем по символам (не по байтам — кириллица!).
    let name: String = body
        .name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let name = if name.is_empty() { "anon".to_string() } else { name };

    if !(0..=MAX_SCORE).contains(&body.score)
        || !(0..=MAX_LINES).contains(&body.lines)
        || !(1..=MAX_LEVEL).contains(&body.level)
    {
        return Err(ApiError::Bad("значения вне допустимого диапазона"));
    }
    // Без единой линии набрать много очков нельзя: только мягкий дроп (≈1–2 очка за клетку).
    if body.lines == 0 && body.score > 2_000 {
        return Err(ApiError::Bad("подозрительный счёт"));
    }

    sqlx::query("INSERT INTO scores (name, score, lines, level) VALUES ($1, $2, $3, $4)")
        .bind(&name)
        .bind(body.score)
        .bind(body.lines)
        .bind(body.level)
        .execute(db)
        .await?;

    let rank: i64 = sqlx::query_scalar("SELECT COUNT(*) + 1 FROM scores WHERE score > $1")
        .bind(body.score)
        .fetch_one(db)
        .await?;

    Ok((StatusCode::CREATED, Json(Saved { rank })))
}

async fn connect_db() -> Option<PgPool> {
    let url = match env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            tracing::warn!("DATABASE_URL не задан — таблица рекордов отключена");
            return None;
        }
    };

    // statement_cache_capacity(0): совместимость с пулерами соединений (PgBouncer у Neon и т.п.).
    let opts = match PgConnectOptions::from_str(&url) {
        Ok(o) => o.statement_cache_capacity(0),
        Err(e) => {
            tracing::error!("неверный DATABASE_URL: {e}");
            return None;
        }
    };

    // Пул маленький: на Vercel каждый инстанс держит свой пул.
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(8))
        .connect_with(opts)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("не удалось подключиться к БД: {e}");
            return None;
        }
    };

    // «Миграции» — идемпотентные, безопасно выполнять при каждом старте.
    let ddl = [
        "CREATE TABLE IF NOT EXISTS scores (
            id         BIGSERIAL PRIMARY KEY,
            name       TEXT        NOT NULL,
            score      INTEGER     NOT NULL,
            lines      INTEGER     NOT NULL,
            level      INTEGER     NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
        "CREATE INDEX IF NOT EXISTS scores_score_idx ON scores (score DESC)",
    ];
    for q in ddl {
        if let Err(e) = sqlx::query(q).execute(&pool).await {
            tracing::error!("миграция не удалась: {e}");
            return None;
        }
    }
    tracing::info!("БД подключена, таблица scores готова");
    Some(pool)
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = term.recv() => {},
            _ = tokio::signal::ctrl_c() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();

    let state = AppState { db: connect_db().await };

    // Vercel передаёт порт через PORT; по умолчанию у контейнера — 80, локально удобнее 3000.
    let port: u16 = env::var("PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    let pkg_dir = env::var("PKG_DIR").unwrap_or_else(|_| "pkg".into());

    let no_store = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );

    let app = Router::new()
        .route("/", get(|| async { Html(SHELL_HTML) }))
        .route("/health", get(|| async { "ok" }))
        .route("/api/scores", get(list_scores).post(add_score))
        .nest_service("/pkg", ServeDir::new(pkg_dir))
        .layer(no_store)
        .layer(CompressionLayer::new())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    tracing::info!("слушаю http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server");
}
