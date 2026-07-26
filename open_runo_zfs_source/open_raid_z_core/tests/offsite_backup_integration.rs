//! 一時退避先(Email/Googleドライブ/SFTP)のモック結合テスト。
//!
//! **実クラウドアカウント・実SMTPサーバー・実VPSへは一切接続しない**
//! ——全てローカルの偽サーバー(モックSMTP・wiremock・インプロセスrussh
//! SFTPサーバー)を使う(CLAUDE.md記載の検証方針どおり)。
#![cfg(feature = "offsite_backup")]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use open_raid_z_core::offsite_backup::{
    EmailBackupTarget, EmailBackupTargetConfig, GoogleDriveBackupTarget, GoogleDriveBackupTargetConfig, OffsiteBackupTarget,
    SftpBackupTarget, SftpBackupTargetConfig,
};

// ============================= Email (モックSMTP) =============================

/// テスト用の最小限のSMTPサーバー。EHLO/AUTH LOGIN/MAIL FROM/RCPT TO/
/// DATA/QUITの一連を受理し、DATA本文を`received`へ記録する。
fn spawn_fake_smtp_server(received: Arc<Mutex<Vec<String>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            handle_smtp_client(stream, Arc::clone(&received));
            // このテストでは1接続だけ処理すれば十分。
            break;
        }
    });

    port
}

fn handle_smtp_client(mut stream: TcpStream, received: Arc<Mutex<Vec<String>>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let _ = stream.write_all(b"220 localhost open-raid-z fake smtp ready\r\n");

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let cmd = line.trim_end();
        if cmd.to_ascii_uppercase().starts_with("EHLO") {
            // 注意: このEHLO応答が広告する認証メカニズムは、下の
            // "AUTH LOGIN"分岐が実装しているチャレンジ方式(LOGIN)のみに
            // 限定すること。以前は"AUTH LOGIN PLAIN"と両方広告していたが、
            // lettreクレートはクライアント側の既定優先順位
            // (`DEFAULT_MECHANISMS = [Plain, Login]`)に従い、サーバーが
            // PLAINも対応していると広告している場合はPLAINを選び単一行の
            // `AUTH PLAIN <base64>`コマンドを送る——この分岐はそれを解釈
            // できず「500 unrecognized command」になっていた(実際に
            // 発生していたテスト失敗の原因)。広告をLOGINのみに絞ることで
            // クライアントに強制的にLOGINを選ばせ、実装済みの
            // チャレンジ・レスポンス処理と一致させる。
            let _ = stream.write_all(b"250-localhost\r\n250-AUTH LOGIN\r\n250 OK\r\n");
        } else if cmd.to_ascii_uppercase().starts_with("AUTH LOGIN") {
            let _ = stream.write_all(b"334 VXNlcm5hbWU6\r\n"); // "Username:"
            line.clear();
            reader.read_line(&mut line).unwrap();
            let _ = stream.write_all(b"334 UGFzc3dvcmQ6\r\n"); // "Password:"
            line.clear();
            reader.read_line(&mut line).unwrap();
            let _ = stream.write_all(b"235 Authentication successful\r\n");
        } else if cmd.to_ascii_uppercase().starts_with("MAIL FROM") {
            let _ = stream.write_all(b"250 OK\r\n");
        } else if cmd.to_ascii_uppercase().starts_with("RCPT TO") {
            let _ = stream.write_all(b"250 OK\r\n");
        } else if cmd.to_ascii_uppercase().starts_with("DATA") {
            let _ = stream.write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n");
            let mut body = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == ".\r\n" {
                    break;
                }
                body.push_str(&line);
            }
            received.lock().unwrap().push(body);
            let _ = stream.write_all(b"250 OK: queued\r\n");
        } else if cmd.to_ascii_uppercase().starts_with("QUIT") {
            let _ = stream.write_all(b"221 Bye\r\n");
            return;
        } else {
            let _ = stream.write_all(b"500 unrecognized command\r\n");
        }
    }
}

#[test]
fn email_backup_target_sends_journal_segment_via_mock_smtp() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let port = spawn_fake_smtp_server(Arc::clone(&received));

    std::env::set_var("TEST_RAIDZ_SMTP_PASSWORD", "unit-test-password");
    let target = EmailBackupTarget::new(EmailBackupTargetConfig {
        smtp_host: "127.0.0.1".to_string(),
        smtp_port: port,
        smtp_username: "backup@example.com".to_string(),
        smtp_password_env: "TEST_RAIDZ_SMTP_PASSWORD".to_string(),
        from_address: "backup@example.com".to_string(),
        to_address: "admin@example.com".to_string(),
        allow_plaintext_for_testing: true,
    });

    target.upload_segment("00000000000000000001.entry.gz", b"fake-compressed-journal-bytes").unwrap();

    // サーバースレッドがDATA受信を処理し終えるまで少し待つ。
    for _ in 0..50 {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let bodies = received.lock().unwrap();
    assert_eq!(bodies.len(), 1, "モックSMTPサーバーが1通のDATAを受信しているべき");
    assert!(bodies[0].contains("00000000000000000001.entry.gz"), "添付ファイル名がメール本文に含まれるべき");
}

// ========================= Google Drive (wiremock) =========================

#[test]
fn google_drive_backup_target_full_roundtrip_via_wiremock() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        // OAuth2リフレッシュトークン->アクセストークン交換。
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "access_token": "fake-access-token" })))
            .mount(&server)
            .await;

        // バックアップフォルダ検索(初回セットアップのfind-or-create、
        // 既に存在する体で応答する)。
        Mock::given(method("GET"))
            .and(path("/drive/v3/files"))
            .and(query_param(
                "q",
                "mimeType='application/vnd.google-apps.folder' and name='open-raid-z-backup' and trashed=false",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "files": [{ "id": "folder-123", "name": "open-raid-z-backup" }]
            })))
            .mount(&server)
            .await;

        // アップロード(multipart)。
        Mock::given(method("POST"))
            .and(path("/upload/drive/v3/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "file-456" })))
            .mount(&server)
            .await;

        // セグメント一覧(自動復帰モードで使用)。
        Mock::given(method("GET"))
            .and(path("/drive/v3/files"))
            .and(query_param("q", "'folder-123' in parents and trashed=false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "files": [{ "id": "file-456", "name": "00000000000000000001.entry.gz" }]
            })))
            .mount(&server)
            .await;

        // ラベル指定でのファイル検索(ダウンロード・削除用)。
        Mock::given(method("GET"))
            .and(path("/drive/v3/files"))
            .and(query_param(
                "q",
                "'folder-123' in parents and name='00000000000000000001.entry.gz' and trashed=false",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "files": [{ "id": "file-456", "name": "00000000000000000001.entry.gz" }]
            })))
            .mount(&server)
            .await;

        // 実データダウンロード。
        Mock::given(method("GET"))
            .and(path("/drive/v3/files/file-456"))
            .and(query_param("alt", "media"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"downloaded-journal-bytes".to_vec()))
            .mount(&server)
            .await;

        // 削除(自動復帰後のクリーンアップ)。
        Mock::given(method("DELETE"))
            .and(path("/drive/v3/files/file-456"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        std::env::set_var("TEST_RAIDZ_GDRIVE_CLIENT_ID", "unit-test-client-id");
        std::env::set_var("TEST_RAIDZ_GDRIVE_CLIENT_SECRET", "unit-test-client-secret");
        std::env::set_var("TEST_RAIDZ_GDRIVE_REFRESH_TOKEN", "unit-test-refresh-token");

        let target = GoogleDriveBackupTarget::new(GoogleDriveBackupTargetConfig {
            backup_folder_name: "open-raid-z-backup".to_string(),
            client_id_env: "TEST_RAIDZ_GDRIVE_CLIENT_ID".to_string(),
            client_secret_env: "TEST_RAIDZ_GDRIVE_CLIENT_SECRET".to_string(),
            refresh_token_env: "TEST_RAIDZ_GDRIVE_REFRESH_TOKEN".to_string(),
            api_base_url_override: Some(server.uri()),
        });

        // ブロッキングクライアント(ureq)呼び出しはtokioランタイム外
        // (別スレッド)で行い、wiremockの非同期サーバーと競合しないようにする。
        let target = std::sync::Arc::new(target);
        let t1 = std::sync::Arc::clone(&target);
        tokio::task::spawn_blocking(move || t1.ensure_ready().unwrap()).await.unwrap();

        let t2 = std::sync::Arc::clone(&target);
        tokio::task::spawn_blocking(move || t2.upload_segment("00000000000000000001.entry.gz", b"unused-in-this-mock").unwrap())
            .await
            .unwrap();

        let t3 = std::sync::Arc::clone(&target);
        let labels = tokio::task::spawn_blocking(move || t3.list_segments().unwrap()).await.unwrap();
        assert_eq!(labels, vec!["00000000000000000001.entry.gz".to_string()]);

        let t4 = std::sync::Arc::clone(&target);
        let downloaded = tokio::task::spawn_blocking(move || t4.download_segment("00000000000000000001.entry.gz").unwrap())
            .await
            .unwrap();
        assert_eq!(downloaded, b"downloaded-journal-bytes".to_vec());

        let t5 = std::sync::Arc::clone(&target);
        tokio::task::spawn_blocking(move || t5.delete_segment("00000000000000000001.entry.gz").unwrap())
            .await
            .unwrap();
    });
}

// ============================= SFTP (インプロセスrussh) =============================

mod fake_sftp_server {
    //! `russh`/`russh_sftp`によるインプロセスの最小SFTPサーバー
    //! (パスワード認証は常に受理、ファイルはテスト用の実tempdirへ
    //! 実際に読み書きする——ネットワーク経由の本物のSFTPプロトコル
    //! ラウンドトリップを検証するため)。
    use russh::keys::{Algorithm, PrivateKey};
    use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
    use russh::{Channel, ChannelId};
    use russh_sftp::protocol::{Attrs, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version};
    use std::collections::HashMap as StdHashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
    use tokio::sync::Mutex;

    #[derive(Clone)]
    pub struct FakeServer {
        pub root: PathBuf,
    }

    impl russh::server::Server for FakeServer {
        type Handler = FakeSshSession;

        fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self::Handler {
            FakeSshSession { root: self.root.clone(), clients: Arc::new(Mutex::new(StdHashMap::new())) }
        }
    }

    pub struct FakeSshSession {
        root: PathBuf,
        clients: Arc<Mutex<StdHashMap<ChannelId, Channel<Msg>>>>,
    }

    impl russh::server::Handler for FakeSshSession {
        type Error = anyhow::Error;

        async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
            Ok(Auth::Accept)
        }

        async fn channel_open_session(
            &mut self,
            channel: Channel<Msg>,
            reply: ChannelOpenHandle,
            _session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.clients.lock().await.insert(channel.id(), channel);
            reply.accept().await;
            Ok(())
        }

        async fn subsystem_request(&mut self, channel_id: ChannelId, name: &str, session: &mut Session) -> Result<(), Self::Error> {
            if name == "sftp" {
                let channel = self.clients.lock().await.remove(&channel_id).unwrap();
                let sftp = FakeSftpSession { root: self.root.clone(), files: StdHashMap::new(), dirs_read: StdHashMap::new() };
                session.channel_success(channel_id)?;
                russh_sftp::server::run(channel.into_stream(), sftp).await;
            } else {
                session.channel_failure(channel_id)?;
            }
            Ok(())
        }
    }

    struct FakeSftpSession {
        root: PathBuf,
        files: StdHashMap<String, tokio::fs::File>,
        dirs_read: StdHashMap<String, bool>,
    }

    impl FakeSftpSession {
        fn resolve(&self, path: &str) -> PathBuf {
            let trimmed = path.trim_start_matches('/');
            self.root.join(trimmed)
        }
    }

    impl russh_sftp::server::Handler for FakeSftpSession {
        type Error = StatusCode;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        async fn init(&mut self, _version: u32, _extensions: StdHashMap<String, String>) -> Result<Version, Self::Error> {
            Ok(Version::new())
        }

        async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
            Ok(Name { id, files: vec![File::dummy(&path)] })
        }

        async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
            let resolved = self.resolve(&path);
            if !resolved.is_dir() {
                return Err(StatusCode::NoSuchFile);
            }
            self.dirs_read.insert(path.clone(), false);
            Ok(Handle { id, handle: path })
        }

        async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
            let done = self.dirs_read.get(&handle).copied().unwrap_or(true);
            if done {
                return Err(StatusCode::Eof);
            }
            self.dirs_read.insert(handle.clone(), true);

            let resolved = self.resolve(&handle);
            let mut files = Vec::new();
            if let Ok(read_dir) = std::fs::read_dir(&resolved) {
                for entry in read_dir.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        files.push(File::new(name, FileAttributes::default()));
                    }
                }
            }
            Ok(Name { id, files })
        }

        async fn mkdir(&mut self, id: u32, path: String, _attrs: FileAttributes) -> Result<Status, Self::Error> {
            let resolved = self.resolve(&path);
            std::fs::create_dir_all(&resolved).map_err(|_| StatusCode::Failure)?;
            Ok(ok_status(id))
        }

        async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
            let resolved = self.resolve(&path);
            if resolved.exists() {
                Ok(Attrs { id, attrs: FileAttributes::default() })
            } else {
                Err(StatusCode::NoSuchFile)
            }
        }

        async fn open(&mut self, id: u32, filename: String, pflags: OpenFlags, _attrs: FileAttributes) -> Result<Handle, Self::Error> {
            let resolved = self.resolve(&filename);
            let write_requested = pflags.contains(OpenFlags::WRITE) || pflags.contains(OpenFlags::CREATE);

            let file = if write_requested {
                tokio::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&resolved)
                    .await
                    .map_err(|_| StatusCode::Failure)?
            } else {
                tokio::fs::OpenOptions::new().read(true).open(&resolved).await.map_err(|_| StatusCode::NoSuchFile)?
            };

            self.files.insert(filename.clone(), file);
            Ok(Handle { id, handle: filename })
        }

        async fn write(&mut self, id: u32, handle: String, offset: u64, data: Vec<u8>) -> Result<Status, Self::Error> {
            let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
            file.seek(std::io::SeekFrom::Start(offset)).await.map_err(|_| StatusCode::Failure)?;
            file.write_all(&data).await.map_err(|_| StatusCode::Failure)?;
            Ok(ok_status(id))
        }

        async fn read(&mut self, id: u32, handle: String, offset: u64, len: u32) -> Result<russh_sftp::protocol::Data, Self::Error> {
            let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
            file.seek(std::io::SeekFrom::Start(offset)).await.map_err(|_| StatusCode::Failure)?;
            let mut buf = vec![0u8; len as usize];
            let n = file.read(&mut buf).await.map_err(|_| StatusCode::Failure)?;
            if n == 0 {
                return Err(StatusCode::Eof);
            }
            buf.truncate(n);
            Ok(russh_sftp::protocol::Data { id, data: buf })
        }

        async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
            self.files.remove(&handle);
            self.dirs_read.remove(&handle);
            Ok(ok_status(id))
        }

        async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
            let resolved = self.resolve(&filename);
            std::fs::remove_file(&resolved).map_err(|_| StatusCode::NoSuchFile)?;
            Ok(ok_status(id))
        }
    }

    fn ok_status(id: u32) -> Status {
        Status { id, status_code: StatusCode::Ok, error_message: "Ok".to_string(), language_tag: "en-US".to_string() }
    }

    pub async fn spawn(root: PathBuf) -> u16 {
        let config = russh::server::Config {
            keys: vec![PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()],
            ..Default::default()
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut server = FakeServer { root };

        tokio::spawn(async move {
            let _ = server.run_on_socket(Arc::new(config), &listener).await;
        });

        port
    }
}

#[test]
fn sftp_backup_target_full_roundtrip_via_inprocess_russh_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    let port = rt.block_on(fake_sftp_server::spawn(root));
    // サーバータスクがacceptループに入るまで少し待つ。
    std::thread::sleep(std::time::Duration::from_millis(100));

    std::env::set_var("TEST_RAIDZ_SFTP_PASSWORD", "unit-test-password");
    let target = SftpBackupTarget::new(SftpBackupTargetConfig {
        host: "127.0.0.1".to_string(),
        port,
        username: "raidz".to_string(),
        password_env: Some("TEST_RAIDZ_SFTP_PASSWORD".to_string()),
        remote_backup_dir: "backup".to_string(),
        known_hosts_path: None,
    });

    target.ensure_ready().unwrap();
    target.upload_segment("00000000000000000001.entry.gz", b"sftp-roundtrip-payload").unwrap();

    let labels = target.list_segments().unwrap();
    assert!(labels.contains(&"00000000000000000001.entry.gz".to_string()));

    let downloaded = target.download_segment("00000000000000000001.entry.gz").unwrap();
    assert_eq!(downloaded, b"sftp-roundtrip-payload".to_vec());

    target.delete_segment("00000000000000000001.entry.gz").unwrap();
    let labels_after_delete = target.list_segments().unwrap();
    assert!(!labels_after_delete.contains(&"00000000000000000001.entry.gz".to_string()));
}

#[test]
fn sftp_host_key_tofu_trusts_first_connection_and_rejects_a_later_mismatched_key() {
    // TOFU(Trust On First Use)方式のホスト鍵検証を実際のknown_hostsファイル
    // (実ファイルシステム上)で検証する: (1) 初回接続は無条件で信頼し記録、
    // (2) 記録済みの鍵と一致する再接続は成功、(3) known_hostsファイルの
    // 記録が(鍵のすり替え等で)実際のサーバーの鍵と一致しなくなった場合は
    // 接続そのものを拒否する、ことを実際のインプロセスSSHサーバーへの
    // 接続で確認する(モックの呼び出し回数確認ではなく、実際の接続成否を見る)。
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("sftp-root");
    std::fs::create_dir_all(&root).unwrap();
    let known_hosts_path = tmp.path().join("known_hosts.txt");

    let port = rt.block_on(fake_sftp_server::spawn(root));
    std::thread::sleep(std::time::Duration::from_millis(100));

    std::env::set_var("TEST_RAIDZ_SFTP_TOFU_PASSWORD", "unit-test-password");
    let make_target = || {
        SftpBackupTarget::new(SftpBackupTargetConfig {
            host: "127.0.0.1".to_string(),
            port,
            username: "raidz".to_string(),
            password_env: Some("TEST_RAIDZ_SFTP_TOFU_PASSWORD".to_string()),
            remote_backup_dir: "backup".to_string(),
            known_hosts_path: Some(known_hosts_path.clone()),
        })
    };

    // (1) 初回接続: known_hostsが存在しない状態から、無条件で信頼して
    // ファイルへ記録することを確認。
    assert!(!known_hosts_path.exists());
    make_target().ensure_ready().expect("first connection should be trusted and recorded (TOFU)");
    let recorded = std::fs::read_to_string(&known_hosts_path).expect("known_hosts file should have been created");
    assert!(recorded.contains(&format!("127.0.0.1:{port} ssh-")), "recorded entry should contain the host:port key and an OpenSSH-formatted public key, got: {recorded}");

    // (2) 記録済みの鍵と一致する再接続は成功する。
    make_target().ensure_ready().expect("reconnecting with the same server key should succeed");

    // (3) known_hostsの記録を、実際のサーバー鍵とは異なる値へ意図的に
    // 書き換える(鍵のすり替え・中間者攻撃の代替)。同じhost:port識別子で、
    // 明らかに異なるダミーのOpenSSH公開鍵行に差し替える。
    let tampered = format!("127.0.0.1:{port} ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINTENTIONALLYWRONGKEYFORTOFUTEST0000000000000\n");
    std::fs::write(&known_hosts_path, tampered).unwrap();

    let result = make_target().ensure_ready();
    assert!(result.is_err(), "connecting when the recorded host key no longer matches the server's actual key must be rejected");
}
