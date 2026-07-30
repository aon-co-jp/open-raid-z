//! open-raid-z Web管理UI(2026-07-30新設)。
//!
//! **技術スタック**: Rust + RPoem(`open-runo-poem-compat`、poem互換API面の
//! 薄いファサード、実体はtokio/hyper直接実装であり`poem`/`tauri`パッケージ
//! への直接依存は無い——ユーザー指示「Rust + RPoem(tokio/hyper直接実装)」
//! への対応)。
//!
//! **アーキテクチャ**: `orzctl`(`open_raid_z_core`のCLIバイナリ、
//! 2026-07-30に`status`サブコマンドを新設済み)をサブプロセスとして呼び出し、
//! JSON出力(Rust-JSON経由)をそのままブラウザへ中継する。プールの
//! 作成(`orzctl create`)は管理者トークン(`OPEN_RAID_Z_ADMIN_TOKEN`環境
//! 変数)必須。`orzctl mount`はフォアグラウンドでFUSE/WinFspループを
//! ブロックする一発コマンドのため、Webリクエストとして呼び出すと
//! ハングしてしまう——**今回のスコープには含めない**(正直な開示)。
//!
//! **read-onlyデモ(rs-syncと同じ設計思想)**: `OPEN_RAID_Z_READ_ONLY=1`
//! 環境変数が設定されている場合、`POST /api/create`は管理者トークンの
//! 有無に関わらず常に拒否する(UI側で作成フォームを隠すだけでなく、
//! サーバー側で確実に強制する——rs-syncの`ReadOnlyGuard`と同じ多層防御)。
//! `/`と`/demo`はどちらも同じ静的ページを返す(相対パスではなく常に
//! 絶対パス`/api/...`でfetchするため、rs-syncで発生した「相対パス+
//! 末尾スラッシュ無し」問題は構造的に発生しない)。

use std::process::Stdio;
use std::sync::Arc;

use open_runo_poem_compat::hyper_compat::{
    self, empty_status, html_response, json_response, read_json_body, Params, Request, Response,
};
use open_runo_poem_compat::{get, post, Route, Server, StatusCode, TcpListener};
use tokio::process::Command;

struct Config {
    orzctl_bin: String,
    level: String,
    chunk_size: String,
    stripes: Option<String>,
    disks: Vec<String>,
    admin_token: Option<String>,
    read_only: bool,
}

impl Config {
    fn from_env() -> Self {
        let disks = std::env::var("OPEN_RAID_Z_DISKS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            orzctl_bin: std::env::var("ORZCTL_BIN").unwrap_or_else(|_| "orzctl".to_string()),
            level: std::env::var("OPEN_RAID_Z_LEVEL").unwrap_or_else(|_| "z2".to_string()),
            chunk_size: std::env::var("OPEN_RAID_Z_CHUNK_SIZE").unwrap_or_else(|_| "4096".to_string()),
            stripes: std::env::var("OPEN_RAID_Z_STRIPES").ok(),
            disks,
            admin_token: std::env::var("OPEN_RAID_Z_ADMIN_TOKEN").ok(),
            read_only: matches!(std::env::var("OPEN_RAID_Z_READ_ONLY").as_deref(), Ok("1") | Ok("true")),
        }
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec!["--level".to_string(), self.level.clone(), "--chunk-size".to_string(), self.chunk_size.clone()];
        if let Some(stripes) = &self.stripes {
            args.push("--stripes".to_string());
            args.push(stripes.clone());
        }
        args.extend(self.disks.iter().cloned());
        args
    }
}

fn page_html(demo: bool) -> String {
    let banner = if demo {
        r#"<div class="banner demo">これはread-onlyデモです。ログイン・登録・保存(プール作成)は実際には出来ません。</div>"#
    } else {
        ""
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>open-raid-z 管理UI</title>
<style>
  body {{ font-family: system-ui, sans-serif; max-width: 720px; margin: 2rem auto; padding: 0 1rem; }}
  .banner.demo {{ background: #fff3cd; border: 1px solid #ffe69c; padding: .75rem 1rem; border-radius: .375rem; margin-bottom: 1rem; }}
  pre {{ background: #f6f8fa; padding: 1rem; border-radius: .375rem; overflow-x: auto; }}
  section {{ margin-top: 1.5rem; }}
  label {{ display: block; margin-bottom: .3rem; font-size: .85rem; color: #555; }}
  input {{ width: 100%; padding: .5rem; margin-bottom: .75rem; box-sizing: border-box; }}
  button {{ padding: .5rem 1rem; cursor: pointer; }}
</style>
</head>
<body>
{banner}
<h1>open-raid-z 管理UI</h1>
<section>
  <h2>プール状態</h2>
  <button id="refresh">更新</button>
  <pre id="status">(未取得)</pre>
</section>
<section id="admin-section">
  <h2>プール作成(管理者のみ)</h2>
  <label for="admin-token">管理者トークン</label>
  <input id="admin-token" type="password" placeholder="X-Admin-Token">
  <label for="dataset-name">データセット名</label>
  <input id="dataset-name" placeholder="例: tank">
  <button id="create">作成</button>
  <pre id="create-result"></pre>
</section>
<script>
async function refreshStatus() {{
  const el = document.getElementById('status');
  el.textContent = '取得中...';
  try {{
    const res = await fetch('/api/status');
    const body = await res.text();
    el.textContent = body;
  }} catch (e) {{
    el.textContent = 'エラー: ' + e;
  }}
}}
document.getElementById('refresh').addEventListener('click', refreshStatus);
document.getElementById('create').addEventListener('click', async () => {{
  const token = document.getElementById('admin-token').value;
  const dataset = document.getElementById('dataset-name').value;
  const el = document.getElementById('create-result');
  el.textContent = '実行中...';
  try {{
    const res = await fetch('/api/create', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json', 'X-Admin-Token': token }},
      body: JSON.stringify({{ dataset }})
    }});
    el.textContent = await res.text();
  }} catch (e) {{
    el.textContent = 'エラー: ' + e;
  }}
}});
refreshStatus();
</script>
</body>
</html>"#
    )
}

async fn run_orzctl(config: &Config, subcommand: &str, extra_args: &[String]) -> Result<(bool, String, String), std::io::Error> {
    let mut args = vec![subcommand.to_string()];
    args.extend(config.base_args());
    args.extend(extra_args.iter().cloned());
    let output = Command::new(&config.orzctl_bin).args(&args).stdin(Stdio::null()).output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((output.status.success(), stdout, stderr))
}

#[derive(serde::Deserialize)]
struct CreateRequest {
    dataset: String,
}

fn main() {
    let config = Arc::new(Config::from_env());

    let index_handler = std::sync::Arc::new(move |_req: Request, _params: Params| {
        Box::pin(async move { html_response(StatusCode::OK, page_html(false)) }) as hyper_compat::BoxFuture<Response>
    }) as hyper_compat::Handler;
    let demo_handler = std::sync::Arc::new(move |_req: Request, _params: Params| {
        Box::pin(async move { html_response(StatusCode::OK, page_html(true)) }) as hyper_compat::BoxFuture<Response>
    }) as hyper_compat::Handler;

    let status_config = Arc::clone(&config);
    let status_handler = std::sync::Arc::new(move |_req: Request, _params: Params| {
        let config = Arc::clone(&status_config);
        Box::pin(async move {
            match run_orzctl(&config, "status", &[]).await {
                Ok((true, stdout, _stderr)) => match rust_json::light::parse_light(&stdout) {
                    Ok(_) => hyper::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(hyper_compat::fixed_body(stdout.into_bytes().into()))
                        .unwrap(),
                    Err(e) => json_response(StatusCode::BAD_GATEWAY, &serde_json::json!({ "error": format!("orzctl status returned invalid JSON: {e:?}") })),
                },
                Ok((false, _stdout, stderr)) => json_response(StatusCode::BAD_GATEWAY, &serde_json::json!({ "error": stderr.trim() })),
                Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, &serde_json::json!({ "error": format!("failed to execute orzctl: {e}") })),
            }
        }) as hyper_compat::BoxFuture<Response>
    }) as hyper_compat::Handler;

    let create_config = Arc::clone(&config);
    let create_handler = std::sync::Arc::new(move |req: Request, _params: Params| {
        let config = Arc::clone(&create_config);
        Box::pin(async move {
            if config.read_only {
                return json_response(StatusCode::FORBIDDEN, &serde_json::json!({ "error": "read-only demo: プール作成は無効化されています / this is a read-only demo, creation is disabled" }));
            }
            let admin_token = match &config.admin_token {
                Some(t) => t.clone(),
                None => return json_response(StatusCode::SERVICE_UNAVAILABLE, &serde_json::json!({ "error": "OPEN_RAID_Z_ADMIN_TOKEN is not configured on this server" })),
            };
            let provided = req.headers().get("x-admin-token").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
            if provided.is_empty() || provided != admin_token {
                return empty_status(StatusCode::UNAUTHORIZED);
            }
            let body: CreateRequest = match read_json_body(req).await {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            match run_orzctl(&config, "create", &["--dataset".to_string(), body.dataset.clone()]).await {
                Ok((true, stdout, _stderr)) => json_response(StatusCode::OK, &serde_json::json!({ "ok": true, "message": stdout.trim() })),
                Ok((false, _stdout, stderr)) => json_response(StatusCode::BAD_GATEWAY, &serde_json::json!({ "ok": false, "error": stderr.trim() })),
                Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, &serde_json::json!({ "ok": false, "error": format!("failed to execute orzctl: {e}") })),
            }
        }) as hyper_compat::BoxFuture<Response>
    }) as hyper_compat::Handler;

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("failed to build tokio runtime");
    rt.block_on(async move {
        let app = Route::new()
            .at("/", get(index_handler))
            .at("/demo", get(demo_handler))
            .at("/api/status", get(status_handler))
            .at("/api/create", post(create_handler));

        let bind_addr: std::net::SocketAddr = std::env::var("OPEN_RAID_Z_WEB_BIND").unwrap_or_else(|_| "127.0.0.1:8099".to_string()).parse().expect("invalid OPEN_RAID_Z_WEB_BIND");
        let (addr, handle) = Server::new(TcpListener::bind(bind_addr)).run(app).await.expect("failed to bind server");
        println!("open-raid-z-web listening on http://{addr} (read_only={})", config.read_only);
        handle.await.ok();
    });
}
