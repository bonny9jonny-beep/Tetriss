use axum::{response::Html, routing::get, Router};
use tower_http::services::ServeDir;

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

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { Html(SHELL_HTML) }))
        .nest_service("/pkg", ServeDir::new("pkg"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Сайт запущен: http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
