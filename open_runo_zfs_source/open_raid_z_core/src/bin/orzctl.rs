//! `orzctl` — open-raid-zのプールを実ディスク(またはループバックファイル)に
//! 対して作成・マウントするための最小限のコマンドラインツール。
//!
//! これまで`Pool`/`mount_pool`/`fuse_mount::mount_pool`はライブラリ関数として
//! しか呼び出せず、実際にコマンドラインから(あるいはinitramfsの起動スクリプト
//! から)呼び出す手段が無かった。このツールはその手段を提供する。
//!
//! 【現状のスコープ】
//! - RAID-Z系vdev(`RaidZVdev`)のみ対応(RAID0/1/5/6/Z2/Z3はすべてこの
//!   `RaidLevel`列挙で表現される。RAID10は別vdevのため未対応)。
//! - `--stripes`は明示的に指定する(生ブロックデバイスの実容量を自動検出
//!   する処理は未実装。`blockdev --getsize64`等で事前に計算すること)。
//! - Linux(`fuse_backend`)専用。他OS/featureでビルドした場合は
//!   起動時にエラーメッセージを出して終了する。

#[cfg(all(target_os = "linux", feature = "fuse_backend"))]
fn real_main() -> Result<(), String> {
    use open_raid_z_core::block_device::FileBackedDevice;
    use open_raid_z_core::fuse_mount::mount_pool;
    use open_raid_z_core::pool::Pool;
    use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};

    let args: Vec<String> = std::env::args().collect();
    let usage = "使い方:\n\
        orzctl create --level <raid0|raid1|raid5|raid6|z2|z3> --chunk-size <バイト> --stripes <総ストライプ数> --dataset <名前> <ディスク1> [ディスク2 ...]\n\
        orzctl mount  --level <raid0|raid1|raid5|raid6|z2|z3> --chunk-size <バイト> --stripes <総ストライプ数> --mountpoint <マウント先ディレクトリ> <ディスク1> [ディスク2 ...]";

    let Some(subcommand) = args.get(1) else {
        return Err(usage.to_string());
    };

    let mut level: Option<RaidLevel> = None;
    let mut chunk_size: Option<usize> = None;
    let mut stripes: Option<u64> = None;
    let mut dataset: Option<String> = None;
    let mut mountpoint: Option<String> = None;
    let mut disks: Vec<String> = Vec::new();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--level" => {
                level = Some(parse_level(args.get(i + 1).ok_or(usage)?)?);
                i += 2;
            }
            "--chunk-size" => {
                chunk_size = Some(args.get(i + 1).ok_or(usage)?.parse().map_err(|_| "chunk-sizeが不正です")?);
                i += 2;
            }
            "--stripes" => {
                stripes = Some(args.get(i + 1).ok_or(usage)?.parse().map_err(|_| "stripesが不正です")?);
                i += 2;
            }
            "--dataset" => {
                dataset = Some(args.get(i + 1).ok_or(usage)?.clone());
                i += 2;
            }
            "--mountpoint" => {
                mountpoint = Some(args.get(i + 1).ok_or(usage)?.clone());
                i += 2;
            }
            other => {
                disks.push(other.to_string());
                i += 1;
            }
        }
    }

    let level = level.ok_or("--levelは必須です")?;
    let chunk_size = chunk_size.ok_or("--chunk-sizeは必須です")?;
    let stripes = stripes.ok_or("--stripesは必須です")?;
    if disks.is_empty() {
        return Err("ディスクを最低1台指定してください".to_string());
    }

    match subcommand.as_str() {
        "create" => {
            let dataset = dataset.ok_or("createには--datasetが必須です")?;
            let devices: Vec<FileBackedDevice> = disks
                .iter()
                .map(|p| FileBackedDevice::open(p).map_err(|e| format!("'{p}'を開けませんでした: {e}")))
                .collect::<Result<_, _>>()?;
            let vdev = RaidZVdev::new(devices, level, chunk_size);
            let mut pool = Pool::new(vdev, stripes);
            pool.create_dataset(&dataset).map_err(|e| format!("データセット作成に失敗: {e}"))?;
            pool.save().map_err(|e| format!("メタデータの保存に失敗: {e}"))?;
            println!("プールを新規作成し、データセット'{dataset}'を作成・保存しました。");
            Ok(())
        }
        "mount" => {
            let mountpoint = mountpoint.ok_or("mountには--mountpointが必須です")?;
            let devices: Vec<FileBackedDevice> = disks
                .iter()
                .map(|p| FileBackedDevice::open(p).map_err(|e| format!("'{p}'を開けませんでした: {e}")))
                .collect::<Result<_, _>>()?;
            let vdev = RaidZVdev::new(devices, level, chunk_size);
            let pool = Pool::open(vdev, stripes).map_err(|e| format!("プールを開けませんでした(保存済みメタデータが無いか、パラメータが保存時と異なります): {e}"))?;
            let session = mount_pool(pool, &mountpoint).map_err(|e| format!("マウントに失敗しました: {e}"))?;
            println!("'{mountpoint}'へマウントしました。Ctrl+Cで終了するとアンマウントされます。");
            // フォアグラウンドで動き続ける(initramfsのスクリプトからは
            // バックグラウンド実行するか、`switch_root`の直前でこのまま
            // 待たせる運用を想定)。
            session.join().map_err(|e| format!("マウントセッションの終了処理に失敗: {e}"))?;
            Ok(())
        }
        _ => Err(usage.to_string()),
    }
}

#[cfg(all(target_os = "linux", feature = "fuse_backend"))]
fn parse_level(s: &str) -> Result<open_raid_z_core::vdev::RaidLevel, String> {
    use open_raid_z_core::vdev::RaidLevel;
    match s.to_ascii_lowercase().as_str() {
        "raid0" => Ok(RaidLevel::Raid0),
        "raid1" => Ok(RaidLevel::Raid1),
        "raid5" => Ok(RaidLevel::Raid5),
        "raid6" => Ok(RaidLevel::Raid6),
        "z2" => Ok(RaidLevel::Z2),
        "z3" => Ok(RaidLevel::Z3),
        other => Err(format!("未知のRAIDレベルです: '{other}'(raid0/raid1/raid5/raid6/z2/z3のいずれか)")),
    }
}

#[cfg(all(target_os = "linux", feature = "fuse_backend"))]
fn main() {
    if let Err(msg) = real_main() {
        eprintln!("{msg}");
        std::process::exit(1);
    }
}

#[cfg(not(all(target_os = "linux", feature = "fuse_backend")))]
fn main() {
    eprintln!("orzctlはLinux上で`fuse_backend` featureを有効にしてビルドした場合のみ動作します。");
    std::process::exit(1);
}
