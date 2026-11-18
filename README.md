# FSociety Tetris (Rust + Axum + Postgres)

Весь код — на Rust:

| Часть | Где | Что делает |
|---|---|---|
| Сервер | `src/main.rs` | Axum: отдаёт страницу и WASM, REST API рекордов, Postgres через sqlx |
| Клиент | `client/src/lib.rs` | Игра, отрисовка, звук, ввод, `fetch` к API — компилируется в WebAssembly |
| Образ | `Dockerfile` | Собирает WASM + сервер, кладёт в минимальный образ |

Единственный JS в проекте — `pkg/*.js`, который **генерирует** wasm-bindgen (браузер не умеет загружать
WASM без этой обвязки), и 3 строки `import init ...` в HTML-оболочке. Руками JS не написан.

## Локальный запуск

### Вариант А — Docker (проще всего)
```bash
docker compose up -d db                       # Postgres
cp .env.example .env
docker build -t tetris .
docker run --rm --network host -e PORT=3000 \
  -e DATABASE_URL=postgres://tetris:tetris@localhost:5432/tetris tetris
```
Открой http://localhost:3000

### Вариант Б — без Docker (Termux, Linux, macOS)
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130   # версия = wasm-bindgen в client/Cargo.lock
docker compose up -d db && cp .env.example .env    # или свой Postgres
./run.sh                                           # http://localhost:3000
```
Без `DATABASE_URL` игра работает, просто таблица рекордов отключена (API отвечает 503).

## API
- `GET  /api/scores?limit=10` → `[{"name","score","lines","level"}, ...]`
- `POST /api/scores` `{"name","score","lines","level"}` → `201 {"rank": N}`
- `GET  /health` → `ok`

Деплой на Vercel — в [DEPLOY.md](DEPLOY.md).
