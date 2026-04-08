# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Antigravity Tools is a Tauri v2 desktop application that manages AI API accounts and provides a local reverse proxy server. It runs as both a desktop app (with system tray) and a headless Docker service. The frontend is a React 19 SPA; the backend is Rust (Tauri + Axum).

## Development Commands

```bash
# Frontend dev server (Vite on port 1420)
npm run dev

# Full desktop app dev (Tauri + Vite)
npm run tauri dev

# Debug mode with Rust logs
npm run tauri:debug    # RUST_LOG=debug npm run tauri dev

# Build frontend only
npm run build          # tsc && vite build

# Build desktop app
npm run tauri build
```

There is no test suite configured. The Rust backend has some tests in `src-tauri/src/proxy/tests/`.

## Architecture

### Dual Runtime Model

The app operates in two modes:
- **Desktop mode**: Tauri v2 app with React webview. The Rust backend manages the window, system tray, and IPC via `tauri::command`.
- **Headless mode**: `--headless` flag skips the Tauri GUI and runs a standalone Tokio runtime with just the proxy server. Used for Docker deployments. Configured via environment variables (`ABV_API_KEY`, `ABV_WEB_PASSWORD`, `ABV_AUTH_MODE`, `ABV_BIND_LOCAL_ONLY`).

### Frontend (`src/`)

- **Framework**: React 19, TypeScript (strict), Vite 7
- **Routing**: React Router v7 — routes defined in `App.tsx`
- **State**: Zustand stores in `src/stores/` (`useAccountStore`, `useConfigStore`, `useViewStore`, `networkMonitorStore`, `useDebugConsole`)
- **UI**: Ant Design + LobeHub UI + DaisyUI + Tailwind CSS v3 (used together)
- **i18n**: i18next with 12 languages in `src/locales/`
- **Pages**: Dashboard, Accounts, ApiProxy, Monitor, TokenStats, UserToken, Security, Settings

#### Unified Request Layer (`src/utils/request.ts`)

The `request()` function abstracts communication:
- **Tauri env**: calls `@tauri-apps/api/core invoke()` directly
- **Web env**: maps Tauri command names to REST API endpoints via `COMMAND_MAPPING` and uses `fetch()`. This enables the same React code to work in both desktop and browser (headless Web UI).

When adding a new Tauri command that needs Web UI support, add the corresponding entry to `COMMAND_MAPPING` in `request.ts`.

### Backend (`src-tauri/`)

- **Language**: Rust 2021 edition
- **Crate name**: `antigravity_tools_lib`
- **Web framework**: Axum 0.7 (serves both the API proxy and the management REST API on port 8045)
- **Database**: SQLite via `rusqlite` (bundled) — separate DBs for token stats, security, user tokens, proxy data
- **HTTP client**: `reqwest` (with rustls-tls, SOCKS proxy) + `rquest` (browser fingerprint impersonation)

Key backend modules:
| Directory | Purpose |
|---|---|
| `src-tauri/src/commands/` | Tauri IPC command handlers (proxy, security, autostart, cloudflared, user_token) |
| `src-tauri/src/proxy/` | Core reverse proxy: Axum server, request handlers, provider mappers, middleware, rate limiting, token management, session management, model specs |
| `src-tauri/src/modules/` | Business logic: account management, config, OAuth, DB access, scheduler, tray, logging, i18n, cloudflared tunnel, migration |
| `src-tauri/src/models/` | Data models (account, config, quota, token) |

### Proxy Architecture (`src-tauri/src/proxy/`)

The proxy is an Axum HTTP server that translates OpenAI-compatible API requests and routes them to upstream AI providers. Key components:
- `server.rs` — Axum router setup, admin API routes, static file serving
- `handlers/` — Request handlers per provider/protocol
- `mappers/` — Request/response format converters between API formats
- `middleware/` — Auth, rate limiting, IP security
- `providers/` — Upstream provider configurations
- `token_manager.rs` — Account rotation and token lifecycle
- `session_manager.rs` — Sticky session support
- `monitor.rs` — Request logging and statistics
- `proxy_pool.rs` — Per-account outbound proxy binding

### Config

App configuration is stored in `gui_config.json` in the platform data directory. Loaded/saved via `modules/config.rs`. The frontend reads/writes config through the `load_config`/`save_config` commands.

## Key Conventions

- Frontend uses `request()` from `src/utils/request.ts` instead of calling `invoke()` directly — this ensures Web mode compatibility.
- Tauri commands are registered in `lib.rs` `invoke_handler`. Each new command also needs a REST API route in `proxy/server.rs` for Web UI support.
- The management API and the AI proxy share the same Axum server on port 8045.
- Vite dev server proxies `/api/` to `http://127.0.0.1:8045` (configured in `vite.config.ts`).
- SQLite databases are initialized at startup in `lib.rs` via `init_db()` calls.
- The app starts with `visible: false` window and calls `show_main_window` from JS to avoid startup flash.
- System tray is auto-disabled on Linux Wayland (can be forced with `ANTIGRAVITY_FORCE_TRAY=1`).
