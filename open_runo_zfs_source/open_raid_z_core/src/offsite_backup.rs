//! 一時退避先(`OffsiteBackupTarget`)抽象化。
//!
//! HDD読み書き中の電源断・USB切断・SATA切断・LAN切断・WiFi切断に
//! 備えて、ジャーナルセグメント(切断直前まで届いていた書き込み内容)を
//! あらかじめ設定した退避先へ複製しておくための抽象化。
//!
//! 対応する退避先(過剰実装を避け、実在し保守されている主要な
//! クレートで実現できるものに限定):
//! - [`EmailBackupTarget`]: SMTP経由でジャーナルセグメントを添付ファイル
//!   として送信する(`lettre`クレート)。**送信専用**(IMAP等での
//!   受信箱からの自動取得は本リポジトリのスコープ外、詳細は下記参照)。
//! - [`GoogleDriveBackupTarget`]: Google Drive REST API v3
//!   (`files.create`/`files.get`/`files.list`)。**このソフトウェアが
//!   ユーザーの代わりにOAuth2認証を行うことは絶対にしない**——
//!   ユーザー自身が発行済みのアクセストークン/リフレッシュトークンを
//!   環境変数経由で受け取るだけ(DuckDNS連携等、既存のエコシステム方針
//!   と同じ)。
//! - [`SftpBackupTarget`]: 汎用SFTP(レンタルサーバー/VPSのバックアップ
//!   フォルダ向け)。`open-web-server`が組み込みSFTPサーバーとして
//!   採用済みの`russh`/`russh-sftp`のクライアント版を流用する。
//!
//! 秘密情報(SMTPパスワード・OAuth2トークン・SFTP認証情報)は、
//! 設定ファイル(TOML)には**環境変数名だけ**を書き、実際の値は
//! 実行時に環境変数から読む(コード・設定ファイルに平文で残さない)。
#![cfg(feature = "offsite_backup")]

use std::io::Read as _;

use serde::{Deserialize, Serialize};

use crate::error::{BridgeError, BridgeResult};

/// 一時退避先の共通インターフェース。
///
/// 実装は「初回セットアップ時に呼ばれる[`ensure_ready`]」
/// 「切断時に呼ばれる[`upload_segment`]」「自動復帰モードで呼ばれる
/// [`list_segments`]/[`download_segment`]/[`delete_segment`]」の
/// 4種類の操作を提供する。全てのメソッドが全ターゲットで意味を持つ
/// わけではない(例: Emailは送信専用)ため、対応しない操作は
/// `BridgeError::NotImplemented`ではなく`OffsiteBackupFailed`で
/// 「このターゲットは対応していません」と正直に返す。
///
/// [`ensure_ready`]: OffsiteBackupTarget::ensure_ready
/// [`upload_segment`]: OffsiteBackupTarget::upload_segment
/// [`list_segments`]: OffsiteBackupTarget::list_segments
/// [`download_segment`]: OffsiteBackupTarget::download_segment
/// [`delete_segment`]: OffsiteBackupTarget::delete_segment
pub trait OffsiteBackupTarget: Send + Sync {
    /// 診断・ログ用のターゲット名(例: `"email:smtp.example.com"`)。
    fn target_name(&self) -> String;

    /// 初回セットアップ時に呼ばれる。退避用フォルダ/宛先の存在確認・
    /// 作成を行う(例: SFTPならリモートディレクトリのmkdir -p、
    /// Googleドライブならバックアップ用フォルダのfind-or-create)。
    /// Emailのように「フォルダ」という概念が無いターゲットは、
    /// 接続確認(SMTP HELOなど)だけ行ってOkを返してよい。
    fn ensure_ready(&self) -> BridgeResult<()>;

    /// ジャーナルセグメント(`label`は`journal/<id>.entry`のような
    /// 識別名)を退避先へ送る。
    fn upload_segment(&self, label: &str, data: &[u8]) -> BridgeResult<()>;

    /// 退避先に残っている(まだローカルへ反映済みでない可能性がある)
    /// セグメントのラベル一覧を返す。自動復帰モードで使用。
    fn list_segments(&self) -> BridgeResult<Vec<String>> {
        Err(BridgeError::OffsiteBackupFailed(format!(
            "{} は一覧取得(自動復帰)に対応していません",
            self.target_name()
        )))
    }

    /// 指定ラベルのセグメント本体を取得する。自動復帰モードで使用。
    fn download_segment(&self, _label: &str) -> BridgeResult<Vec<u8>> {
        Err(BridgeError::OffsiteBackupFailed(format!(
            "{} はダウンロード(自動復帰)に対応していません",
            self.target_name()
        )))
    }

    /// 自動復帰モードでローカルへの反映が完了した後、退避先に残った
    /// セグメントを片付ける(運用ポリシー: 反映済みセグメントを退避先に
    /// 無期限に残さないための整理。失敗しても致命的ではない)。
    fn delete_segment(&self, _label: &str) -> BridgeResult<()> {
        Ok(())
    }
}

/// 環境変数からのみ秘密情報を読み込むヘルパー。
/// (このエコシステムの既存方針: 秘密情報は環境変数経由、平文保存禁止)
fn read_secret_env(var_name: &str) -> BridgeResult<String> {
    std::env::var(var_name).map_err(|_| {
        BridgeError::OffsiteBackupFailed(format!(
            "環境変数 {var_name} が設定されていません(秘密情報は環境変数経由で渡す設計のため)"
        ))
    })
}

// ============================= Email =============================

/// SMTP経由でジャーナルセグメントをメール添付として転送する退避先。
/// 送信専用(受信箱からの自動取得はスコープ外、下記`list_segments`参照)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailBackupTargetConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    /// SMTPパスワードを保持する環境変数名(値そのものは書かない)。
    pub smtp_password_env: String,
    pub from_address: String,
    pub to_address: String,
    /// テスト・ローカルSMTPリレー向け: trueならSTARTTLS/TLSを使わない
    /// 平文接続(実運用では必ずfalseにすること)。
    #[serde(default)]
    pub allow_plaintext_for_testing: bool,
}

pub struct EmailBackupTarget {
    config: EmailBackupTargetConfig,
}

impl EmailBackupTarget {
    pub fn new(config: EmailBackupTargetConfig) -> Self {
        Self { config }
    }

    fn build_transport(&self) -> BridgeResult<lettre::SmtpTransport> {
        use lettre::transport::smtp::authentication::Credentials;

        let password = read_secret_env(&self.config.smtp_password_env)?;
        let creds = Credentials::new(self.config.smtp_username.clone(), password);

        let builder = if self.config.allow_plaintext_for_testing {
            lettre::SmtpTransport::builder_dangerous(&self.config.smtp_host)
        } else {
            lettre::SmtpTransport::starttls_relay(&self.config.smtp_host)
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SMTPリレー構築失敗: {e}")))?
        };

        Ok(builder.port(self.config.smtp_port).credentials(creds).build())
    }
}

impl OffsiteBackupTarget for EmailBackupTarget {
    fn target_name(&self) -> String {
        format!("email:{}", self.config.smtp_host)
    }

    fn ensure_ready(&self) -> BridgeResult<()> {
        use lettre::transport::smtp::SmtpTransport;

        let transport: SmtpTransport = self.build_transport()?;
        transport
            .test_connection()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SMTP接続確認失敗: {e}")))?;
        Ok(())
    }

    fn upload_segment(&self, label: &str, data: &[u8]) -> BridgeResult<()> {
        use lettre::message::header::ContentType;
        use lettre::message::{Attachment, MultiPart, SinglePart};
        use lettre::{Message, Transport};

        let email = Message::builder()
            .from(self.config.from_address.parse().map_err(|e| {
                BridgeError::OffsiteBackupFailed(format!("差出人アドレスが不正です: {e}"))
            })?)
            .to(self.config.to_address.parse().map_err(|e| {
                BridgeError::OffsiteBackupFailed(format!("宛先アドレスが不正です: {e}"))
            })?)
            .subject(format!("[open-raid-z] 切断時ジャーナル退避: {label}"))
            .multipart(
                MultiPart::mixed()
                    .singlepart(SinglePart::plain(format!(
                        "open-raid-zの切断耐性ジャーナルセグメントです。\n\
                         ラベル: {label}\n\
                         サイズ: {} bytes\n\
                         (自動復帰モードには対応していません。手動でIMAP等から取得・保存してください)",
                        data.len()
                    )))
                    .singlepart(Attachment::new(label.to_string()).body(data.to_vec(), ContentType::parse("application/octet-stream").unwrap())),
            )
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("メール構築失敗: {e}")))?;

        let transport = self.build_transport()?;
        transport
            .send(&email)
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("メール送信失敗: {e}")))?;
        Ok(())
    }

    // list_segments/download_segmentは既定実装のまま(未対応)。
    // 理由: メールは受信箱の検索・添付抽出にIMAP等の別プロトコルが
    // 必要で、このリポジトリのスコープ(過剰実装を避ける方針)を
    // 超えるため。自動復帰モードはSFTP/Googleドライブのみ対応する。
}

// ========================= Google Drive =========================

/// Google Drive REST API v3経由の退避先。
///
/// **重要**: このソフトウェアはOAuth2認証フロー自体を代行しない。
/// ユーザーが事前にGoogle Cloud Consoleでクライアント登録・同意画面を
/// 通過して取得済みの`refresh_token`/`client_id`/`client_secret`を
/// 環境変数として渡すだけであり、初回のトークン発行(ブラウザでの
/// ログイン・同意)はユーザー自身が行う前提(DuckDNS連携等と同じ、
/// エコシステム既存方針)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveBackupTargetConfig {
    /// バックアップ先フォルダ名(無ければ`ensure_ready`で作成する)。
    pub backup_folder_name: String,
    /// OAuth2 クライアントID を保持する環境変数名。
    pub client_id_env: String,
    /// OAuth2 クライアントシークレットを保持する環境変数名。
    pub client_secret_env: String,
    /// ユーザー自身が事前取得済みのリフレッシュトークンを保持する環境変数名。
    pub refresh_token_env: String,
    /// テスト用: `https://www.googleapis.com`の代わりに使う起点URL
    /// (wiremockモックサーバーのURLを指す。本番では`None`のままでよい)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url_override: Option<String>,
}

pub struct GoogleDriveBackupTarget {
    config: GoogleDriveBackupTargetConfig,
    folder_id: std::sync::Mutex<Option<String>>,
}

impl GoogleDriveBackupTarget {
    pub fn new(config: GoogleDriveBackupTargetConfig) -> Self {
        Self { config, folder_id: std::sync::Mutex::new(None) }
    }

    fn api_base(&self) -> String {
        self.config
            .api_base_url_override
            .clone()
            .unwrap_or_else(|| "https://www.googleapis.com".to_string())
    }

    fn oauth_token_base(&self) -> String {
        self.config
            .api_base_url_override
            .clone()
            .unwrap_or_else(|| "https://oauth2.googleapis.com".to_string())
    }

    /// リフレッシュトークンからアクセストークンを取得する
    /// (単発のHTTP POST。OAuth2「認可コード取得」フロー自体はユーザーが
    /// 別途完了済みという前提——ここではその後段の「更新」だけを行う)。
    fn fetch_access_token(&self) -> BridgeResult<String> {
        let client_id = read_secret_env(&self.config.client_id_env)?;
        let client_secret = read_secret_env(&self.config.client_secret_env)?;
        let refresh_token = read_secret_env(&self.config.refresh_token_env)?;

        let url = format!("{}/token", self.oauth_token_base());
        let resp: serde_json::Value = ureq::post(&url)
            .send_form([
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleトークン更新リクエスト失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleトークン応答の解析失敗: {e}")))?;

        resp.get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::OffsiteBackupFailed("Googleトークン応答にaccess_tokenがありません".to_string()))
    }

    fn resolve_folder_id(&self, access_token: &str) -> BridgeResult<String> {
        if let Some(id) = self.folder_id.lock().unwrap().clone() {
            return Ok(id);
        }

        let query = format!(
            "mimeType='application/vnd.google-apps.folder' and name='{}' and trashed=false",
            self.config.backup_folder_name.replace('\'', "\\'")
        );
        let url = format!("{}/drive/v3/files", self.api_base());
        let resp: serde_json::Value = ureq::get(&url)
            .header("Authorization", &format!("Bearer {access_token}"))
            .query("q", &query)
            .call()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブfiles.list失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答の解析失敗: {e}")))?;

        if let Some(id) = resp
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .and_then(|f| f.get("id"))
            .and_then(|v| v.as_str())
        {
            *self.folder_id.lock().unwrap() = Some(id.to_string());
            return Ok(id.to_string());
        }

        // 見つからなければ新規作成(初回セットアップフローでの
        // 「バックアップフォルダを作成」動作)。
        let create_url = format!("{}/drive/v3/files", self.api_base());
        let created: serde_json::Value = ureq::post(&create_url)
            .header("Authorization", &format!("Bearer {access_token}"))
            .send_json(serde_json::json!({
                "name": self.config.backup_folder_name,
                "mimeType": "application/vnd.google-apps.folder",
            }))
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブフォルダ作成失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答の解析失敗: {e}")))?;

        let id = created
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BridgeError::OffsiteBackupFailed("Googleドライブフォルダ作成応答にidがありません".to_string()))?
            .to_string();
        *self.folder_id.lock().unwrap() = Some(id.clone());
        Ok(id)
    }
}

impl OffsiteBackupTarget for GoogleDriveBackupTarget {
    fn target_name(&self) -> String {
        format!("google_drive:{}", self.config.backup_folder_name)
    }

    fn ensure_ready(&self) -> BridgeResult<()> {
        let token = self.fetch_access_token()?;
        self.resolve_folder_id(&token)?;
        Ok(())
    }

    fn upload_segment(&self, label: &str, data: &[u8]) -> BridgeResult<()> {
        let token = self.fetch_access_token()?;
        let folder_id = self.resolve_folder_id(&token)?;

        // multipart/related によるメタデータ+バイナリの単発アップロード
        // (Google Drive APIの標準的な小容量アップロード手順)。
        let boundary = "open_raid_z_offsite_boundary";
        let metadata = serde_json::json!({ "name": label, "parents": [folder_id] }).to_string();
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n").as_bytes());
        body.extend_from_slice(format!("--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

        let url = format!("{}/upload/drive/v3/files?uploadType=multipart", self.api_base());
        ureq::post(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Content-Type", &format!("multipart/related; boundary={boundary}"))
            .send(&body)
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブアップロード失敗: {e}")))?;
        Ok(())
    }

    fn list_segments(&self) -> BridgeResult<Vec<String>> {
        let token = self.fetch_access_token()?;
        let folder_id = self.resolve_folder_id(&token)?;

        let url = format!("{}/drive/v3/files", self.api_base());
        let resp: serde_json::Value = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .query("q", &format!("'{folder_id}' in parents and trashed=false"))
            .call()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブfiles.list失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答の解析失敗: {e}")))?;

        Ok(resp
            .get("files")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| f.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default())
    }

    fn download_segment(&self, label: &str) -> BridgeResult<Vec<u8>> {
        let token = self.fetch_access_token()?;
        let folder_id = self.resolve_folder_id(&token)?;

        let url = format!("{}/drive/v3/files", self.api_base());
        let resp: serde_json::Value = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .query("q", &format!("'{folder_id}' in parents and name='{label}' and trashed=false"))
            .call()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブfiles.list失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答の解析失敗: {e}")))?;

        let file_id = resp
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .and_then(|f| f.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| BridgeError::OffsiteBackupFailed(format!("Googleドライブに{label}が見つかりません")))?;

        let download_url = format!("{}/drive/v3/files/{file_id}", self.api_base());
        let mut resp = ureq::get(&download_url)
            .header("Authorization", &format!("Bearer {token}"))
            .query("alt", "media")
            .call()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブダウンロード失敗: {e}")))?;

        let mut buf = Vec::new();
        resp.body_mut()
            .as_reader()
            .read_to_end(&mut buf)
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答本文の読み取り失敗: {e}")))?;
        Ok(buf)
    }

    fn delete_segment(&self, label: &str) -> BridgeResult<()> {
        let token = self.fetch_access_token()?;
        let folder_id = self.resolve_folder_id(&token)?;

        let url = format!("{}/drive/v3/files", self.api_base());
        let resp: serde_json::Value = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .query("q", &format!("'{folder_id}' in parents and name='{label}' and trashed=false"))
            .call()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブfiles.list失敗: {e}")))?
            .body_mut()
            .read_json()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ応答の解析失敗: {e}")))?;

        if let Some(file_id) = resp
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .and_then(|f| f.get("id"))
            .and_then(|v| v.as_str())
        {
            let delete_url = format!("{}/drive/v3/files/{file_id}", self.api_base());
            ureq::delete(&delete_url)
                .header("Authorization", &format!("Bearer {token}"))
                .call()
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("Googleドライブ削除失敗: {e}")))?;
        }
        Ok(())
    }
}

// =============================== SFTP ===============================

/// 汎用SFTP(レンタルサーバー/VPSのバックアップフォルダ向け)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpBackupTargetConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// パスワード認証を使う場合の環境変数名(公開鍵認証を使う場合はNone)。
    #[serde(default)]
    pub password_env: Option<String>,
    /// リモートのバックアップ先ディレクトリ(無ければ`ensure_ready`で
    /// 作成する)。
    pub remote_backup_dir: String,
}

pub struct SftpBackupTarget {
    config: SftpBackupTargetConfig,
}

struct SftpPasswordAuthHandler;

impl russh::client::Handler for SftpPasswordAuthHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &russh::keys::PublicKey) -> Result<bool, Self::Error> {
        // このリポジトリのSFTPターゲットは「ユーザー自身が設定したホスト」
        // への一時退避専用であり、既知ホスト鍵の永続的な検証は
        // 今回のスコープ外(2026-07時点で未検証の項目としてCLAUDE.md
        // HANDOFFに正直に記録する)。
        Ok(true)
    }
}

impl SftpBackupTarget {
    pub fn new(config: SftpBackupTargetConfig) -> Self {
        Self { config }
    }

    /// SFTP呼び出しは非同期(tokio/russh前提)だが、他の退避先と
    /// APIの一貫性を保つため、専用の使い捨てtokioランタイム上で
    /// 同期関数として実行する。
    fn run_sync<F, T>(&self, fut: F) -> BridgeResult<T>
    where
        F: std::future::Future<Output = BridgeResult<T>>,
    {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTP用tokioランタイム起動失敗: {e}")))?;
        rt.block_on(fut)
    }

    async fn connect_session(&self) -> BridgeResult<russh_sftp::client::SftpSession> {
        let password = match &self.config.password_env {
            Some(var) => read_secret_env(var)?,
            None => {
                return Err(BridgeError::OffsiteBackupFailed(
                    "SFTP: password_envが未設定です(公開鍵認証は今回未対応、2026-07時点の既知の制約)".to_string(),
                ))
            }
        };

        let config = russh::client::Config::default();
        let handler = SftpPasswordAuthHandler;
        let mut session = russh::client::connect(std::sync::Arc::new(config), (self.config.host.as_str(), self.config.port), handler)
            .await
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SSH接続失敗: {e}")))?;

        let authenticated = session
            .authenticate_password(&self.config.username, &password)
            .await
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SSHパスワード認証失敗: {e}")))?;
        if !authenticated.success() {
            return Err(BridgeError::OffsiteBackupFailed("SSHパスワード認証を拒否されました".to_string()));
        }

        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SSHチャンネル開設失敗: {e}")))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTPサブシステム要求失敗: {e}")))?;

        russh_sftp::client::SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTPセッション確立失敗: {e}")))
    }

    fn remote_path(&self, label: &str) -> String {
        format!("{}/{}", self.config.remote_backup_dir.trim_end_matches('/'), label)
    }
}

impl OffsiteBackupTarget for SftpBackupTarget {
    fn target_name(&self) -> String {
        format!("sftp:{}@{}:{}", self.config.username, self.config.host, self.config.remote_backup_dir)
    }

    fn ensure_ready(&self) -> BridgeResult<()> {
        self.run_sync(async move {
            let sftp = self.connect_session().await?;
            // 既に存在すればOk(既存ディレクトリ)、無ければ作成する
            // (初回セットアップ時の「バックアップフォルダ作成」動作)。
            match sftp.create_dir(&self.config.remote_backup_dir).await {
                Ok(()) => Ok(()),
                Err(_) => match sftp.metadata(&self.config.remote_backup_dir).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(BridgeError::OffsiteBackupFailed(format!("SFTPバックアップフォルダ作成/確認失敗: {e}"))),
                },
            }
        })
    }

    fn upload_segment(&self, label: &str, data: &[u8]) -> BridgeResult<()> {
        let data = data.to_vec();
        let path = self.remote_path(label);
        self.run_sync(async move {
            use russh_sftp::protocol::OpenFlags;
            use tokio::io::AsyncWriteExt as _;

            let sftp = self.connect_session().await?;
            // `SftpSession::write()`(の便利ラッパー)は内部で
            // `AsyncWriteExt::write_all`のみ呼び、書き込み確認応答(ack)の
            // 完了待ち・SSH_FXP_CLOSE送信を行わずに戻ってしまうため、
            // このメソッド呼び出しが返った時点でリモート側へ確実に反映されている
            // 保証が無い(直後の再オープン読み取りで空データが返ることがある、
            // 実際にテストで再現した既知の問題)。そのため明示的に
            // open→write_all→shutdown(flush+SSH_FXP_CLOSEの完了待ち)の順で
            // 呼び出し、書き込みが確実に完了・クローズされてから戻るようにする。
            let mut file = sftp
                .open_with_flags(path, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTP書き込み用オープン失敗: {e}")))?;
            file.write_all(&data)
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTP書き込み失敗: {e}")))?;
            file.shutdown()
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTPファイルクローズ失敗: {e}")))?;
            Ok(())
        })
    }

    fn list_segments(&self) -> BridgeResult<Vec<String>> {
        self.run_sync(async move {
            let sftp = self.connect_session().await?;
            let entries = sftp
                .read_dir(&self.config.remote_backup_dir)
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTPディレクトリ一覧取得失敗: {e}")))?;
            Ok(entries.map(|e| e.file_name()).collect())
        })
    }

    fn download_segment(&self, label: &str) -> BridgeResult<Vec<u8>> {
        let label = label.to_string();
        self.run_sync(async move {
            let sftp = self.connect_session().await?;
            sftp.read(self.remote_path(&label))
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTP読み取り失敗: {e}")))
        })
    }

    fn delete_segment(&self, label: &str) -> BridgeResult<()> {
        let label = label.to_string();
        self.run_sync(async move {
            let sftp = self.connect_session().await?;
            sftp.remove_file(self.remote_path(&label))
                .await
                .map_err(|e| BridgeError::OffsiteBackupFailed(format!("SFTP削除失敗: {e}")))?;
            Ok(())
        })
    }
}
