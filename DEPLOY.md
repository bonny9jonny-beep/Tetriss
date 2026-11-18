# Деплой на Vercel: полный разбор

Идея: твой Axum-сервер — это обычный долгоживущий HTTP-процесс. Vercel умеет запускать такое как
**Container Runtime** (запускает Docker-образ как Vercel Function, на Fluid compute: масштабируется
по трафику и «засыпает» в простое). Поэтому мы упаковываем сервер в Docker-образ, а Vercel его строит и запускает.
Базу берём внешнюю (Neon Postgres через Marketplace), потому что файловая система функции не сохраняется
между запусками — SQLite-файл там бы пропадал.

> ⚠️ Container Runtime у Vercel сейчас в **Beta** (доступен на всех тарифах). Детали и лимиты могут меняться:
> https://vercel.com/docs/functions/runtimes/container и гайд
> https://vercel.com/kb/guide/deploy-rust-on-vercel-with-docker

---

## Шаг 0. Что понадобится
- Аккаунт на https://vercel.com и на https://github.com
- Git
- (Для проверки локально) Docker — по желанию; сам деплой Docker на твоей машине не требует, образ строит Vercel

## Шаг 1. Положи проект на GitHub
```bash
cd fsociety-tetris
git init
git add .
git commit -m "FSociety Tetris: Rust + Axum + Postgres"
# создай пустой репозиторий на github.com (без README), затем:
git remote add origin https://github.com/<ты>/fsociety-tetris.git
git branch -M main
git push -u origin main
```
Проверь, что в репозитории есть **оба** `Cargo.lock` (корневой и `client/Cargo.lock`) —
Docker-сборка использует `--locked` и версию wasm-bindgen берёт из `client/Cargo.lock`.
`pkg/` и `target/` в репозиторий не попадают (они в `.gitignore`) — `pkg/` собирается внутри Docker.

## Шаг 2. Импортируй проект в Vercel
1. https://vercel.com/new → **Import Git Repository** → выбери `fsociety-tetris`.
2. Vercel увидит `Dockerfile` в корне, выберет пресет **Container** и направит весь трафик в образ.
   Ничего в Build/Output настройках менять не надо.
3. Пока **не жми Deploy** — сначала подключим базу (иначе первый деплой пройдёт без рекордов; это не страшно,
   можно и так, потом сделать Redeploy).

   Если жмёшь Deploy сразу и проект уже существовал с другим фреймворком, выполни в папке проекта:
   `vercel project update --framework container --yes`.

## Шаг 3. Подключи Postgres (Neon)
1. В проекте Vercel: вкладка **Storage** → **Create Database** (или Marketplace → **Neon**).
   Название у провайдера в Marketplace может слегка отличаться — ищи «Postgres».
2. Выбери бесплатный план и регион (**ближе к функции Vercel**; для Европы — Frankfurt `fra1`/ближайший).
3. **Connect Project** → выбери проект и окружения: **Production** и **Preview** (Development — по желанию).
4. Vercel сам добавит переменные окружения, главная — **`DATABASE_URL`**. Проверь:
   *Settings → Environment Variables*.

Наш сервер читает именно `DATABASE_URL` и при старте сам создаёт таблицу `scores` (`CREATE TABLE IF NOT EXISTS`) —
ручные миграции не нужны.

## Шаг 4. Deploy
Нажми **Deploy** (или сделай `git push` — каждый пуш в `main` деплоится автоматически,
пуши в другие ветки дают Preview-деплой со своим URL).

Что происходит при сборке (см. `Dockerfile`):
1. **Этап `wasm`** — `rust:1-bookworm` + target `wasm32-unknown-unknown`, компилирует `client/` в `.wasm`;
   затем скачивает `wasm-bindgen` **той же версии**, что в `client/Cargo.lock`, и генерирует `pkg/`.
2. **Этап `server`** — компилирует Axum-сервер в один бинарник. Сначала собираются только зависимости
   (пустой `main.rs`), потом ваш код — так Docker переиспользует кэш, и повторные сборки быстрые.
3. **Финальный образ** — `distroless/cc-debian12`: только бинарник + `pkg/`, без компилятора, запуск не от root.
   Debian-сборка + Debian-рантайм = совместимая glibc (смешивать Alpine и Debian нельзя — будет «missing loader»).

Первая сборка занимает несколько минут (компиляция Rust с нуля).

## Шаг 5. Проверка
Открой выданный URL `https://<проект>.vercel.app`:
```bash
curl https://<проект>.vercel.app/health            # ok
curl https://<проект>.vercel.app/api/scores        # [] на пустой базе
```
Сыграй партию → на экране «Стек переполнен» введи ник → «В таблицу» → увидишь своё место и топ-5.

Логи: проект → **Logs** (stdout/stderr контейнера). При старте должна быть строка
`БД подключена, таблица scores готова`.

## Шаг 6. Свой домен (по желанию)
*Settings → Domains → Add* → введи домен → добавь DNS-записи, которые покажет Vercel.

---

## Деплой через CLI (без GitHub)
```bash
npm i -g vercel
vercel login
vercel link                 # привязать/создать проект
# подключи Neon в дашборде (Шаг 3), затем:
vercel env pull .env        # подтянуть DATABASE_URL для локальной разработки
vercel deploy --prod
```
Локально в контейнерном окружении Vercel: `vercel dev -L` (нужен Docker).

---

## Разбор: как устроен код

### Сервер `src/main.rs`
- **Порт.** Читает `PORT` (в образе `ENV PORT=80` — это порт по умолчанию, куда Vercel шлёт трафик) и слушает `0.0.0.0`.
  Если привязать к `127.0.0.1` — будет 502.
- **Graceful shutdown.** Vercel при «засыпании» шлёт `SIGTERM` и даёт 30 секунд — сервер завершается корректно.
- **Пул соединений.** `PgPoolOptions::max_connections(5)`: на serverless каждый инстанс держит свой пул,
  большой пул быстро исчерпает лимит соединений базы.
- **`statement_cache_capacity(0)`.** Кэш prepared statements выключен — это страховка совместимости с пулерами
  (PgBouncer, у Neon pooled-строка идёт через него).
- **TLS.** Фича sqlx `tls-rustls-ring-webpki` — шифрование до Neon без системных библиотек.
- **Валидация.** Ник: управляющие символы вырезаются, обрезка до 16 *символов* (не байт — кириллица), пусто → `anon`.
  Счёт/линии/уровень ограничены. Счёт > 2000 при 0 линий отклоняется.
- **Без БД не падает.** Нет `DATABASE_URL` → игра работает, `/api/scores` отвечает 503.

### Клиент `client/src/lib.rs`
- `api_get_scores` / `api_post_score` — `fetch` через `web-sys` (никакого JS).
- Таблица рисуется через `set_text_content` (не `innerHTML`) — ник игрока не может внедрить HTML/скрипт (XSS).
- Клавиатурный обработчик игнорирует нажатия, пока фокус в поле ника (иначе пробел/стрелки/`r` управляли бы игрой).

### Версии wasm-bindgen
Крейт `wasm-bindgen` в `client/Cargo.lock` и CLI `wasm-bindgen` **обязаны совпадать по версии**, иначе при загрузке
будет ошибка схемы. Dockerfile берёт версию из lock-файла сам. Обновляешь зависимости — просто пересобираешь образ.

---

## Частые проблемы
| Симптом | Причина → решение |
|---|---|
| 502 на всех запросах | Сервер слушает не `0.0.0.0` или не тот порт → в образе `PORT=80`, не переопределяй переменную `PORT` в Vercel на другое значение без нужды |
| «missing loader / library» | Смешаны Alpine и Debian в Dockerfile → оставь `rust:1-bookworm` + `distroless/cc-debian12` |
| Игра грузится, но «В таблицу» → «Ошибка» | Нет `DATABASE_URL` (ответ 503) или БД недоступна → проверь Settings → Environment Variables и Logs |
| `/api/scores` отвечает 503 | Переменная не подключена к нужному окружению (Production/Preview) → *Storage → Connect Project*, затем Redeploy |
| Сборка падает на wasm-bindgen | Нет сети до GitHub в сборке или не закоммичен `client/Cargo.lock` → закоммить lock |
| Долгая сборка | Нормально для первой; дальше помогает кэш слоёв. Не копируй исходники до этапа «только зависимости» |
| Первый запрос после простоя медленный | Холодный старт: в продакшене без трафика 5 минут инстанс засыпает (в Preview — 30 секунд) |

## Ограничения (честно)
- **Контейнерный рантайм Vercel — Beta**, условия/цены уточняй в дашборде и документации.
- **Рекорды можно подделать**: счёт считает клиент (WASM), сервер проверяет только правдоподобие. Для честного
  лидерборда пришлось бы переносить игровую логику на сервер (проверка по записи ходов). Для друзей/демо этого достаточно.
- Нет защиты от спама (rate limit). Если выложишь публично — добавь `tower_governor` или лимит на уровне Vercel Firewall.
