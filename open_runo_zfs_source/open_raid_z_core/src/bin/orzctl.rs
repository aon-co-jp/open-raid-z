//! `orzctl` — open-raid-zのプールを実ディスク(またはループバックファイル)に
//! 対して作成・マウントするための、Windows/Linux共通のコマンドラインツール。
//!
//! これまで`Pool`/`mount::mount_pool`(Windows/WinFsp)/`fuse_mount::mount_pool`
//! (Linux/FUSE)はライブラリ関数としてしか呼び出せず、実際にコマンドライン
//! から(あるいはinitramfsの起動スクリプトから)呼び出す手段が無かった。
//! このツールはその手段を提供する。
//!
//! 【対応環境】
//! - `create`(プール新規作成)はOS非依存で常に使える。
//! - `mount`(実マウント)は、Windowsでは`winfsp_backend` feature、Linuxでは
//!   `fuse_backend` featureがビルドに含まれている場合のみ使える(それ以外の
//!   ビルドでは`mount`サブコマンド自体がエラーになる)。
//! - Linuxのシェルからも、WindowsのPowerShellからも、全く同じコマンド・
//!   オプション名で操作できる(OSによる違いはマウント先の指定方法だけ:
//!   Linuxはディレクトリパス、Windowsはドライブレター文字列)。
//!
//! 【現状のスコープ】
//! - RAID-Z系vdev(`RaidZVdev`)のみ対応(RAID0/1/5/6/Z2/Z3はすべてこの
//!   `RaidLevel`列挙で表現される。RAID10は別vdevのため未対応)。
//! - `--stripes`は明示的に指定する(生ブロックデバイスの実容量を自動検出
//!   する処理は未実装。Linuxでは`blockdev --getsize64`、Windowsでは
//!   `Get-Disk`/`Get-Partition`等で事前に計算すること)。

use open_raid_z_core::block_device::FileBackedDevice;
use open_raid_z_core::pool::Pool;
use open_raid_z_core::vdev::{RaidLevel, RaidZVdev};

const HELP: &str = r#"orzctl - open-raid-z のプールを作成・マウントするコマンドラインツール

使い方:
  orzctl create --level <LEVEL> --chunk-size <BYTES> --stripes <N> --dataset <NAME> <DISK...>
  orzctl mount  --level <LEVEL> --chunk-size <BYTES> --stripes <N> --mountpoint <PATH> <DISK...>
  orzctl help | --help | -h

サブコマンド:
  create    実ディスク(またはループバックファイル)から新しいプールを作成し、
            指定した名前のデータセットを1つ作成して保存する。
  mount     保存済みのプールを開き、実際にファイルシステムとしてマウントする。
            Linuxではディレクトリへ、Windowsではドライブレター(例: "Z:")へ
            マウントする。マウント中はプロセスがフォアグラウンドで待機し、
            Linuxでは他のターミナルから`fusermount3 -u <PATH>`されるまで、
            Windowsでは標準入力へEnterキーが押されるまでアンマウントしない。
  help      このヘルプを表示する。

オプション(create/mount共通):
  --level <LEVEL>       raid0 | raid1 | raid5 | raid6 | z2 | z3
  --chunk-size <BYTES>  1ディスクあたりのチャンクサイズ(バイト)
  --stripes <N>         プールの総ストライプ数(全ディスク共通)

createのみ:
  --dataset <NAME>      作成するデータセットの名前

mountのみ:
  --mountpoint <PATH>   Linux: マウント先ディレクトリ(既存の空ディレクトリ)
                        Windows: ドライブレター(例: "Z:")

例(Linux、シェル):
  orzctl create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg
  orzctl mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint /mnt/tank /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

例(Windows、PowerShell。同じコマンド・同じオプション名で操作できる):
  orzctl.exe create --level z2 --chunk-size 4096 --stripes 1000 --dataset tank \\.\PhysicalDrive1 \\.\PhysicalDrive2 \\.\PhysicalDrive3 \\.\PhysicalDrive4 \\.\PhysicalDrive5 \\.\PhysicalDrive6
  orzctl.exe mount  --level z2 --chunk-size 4096 --stripes 1000 --mountpoint Z: \\.\PhysicalDrive1 \\.\PhysicalDrive2 \\.\PhysicalDrive3 \\.\PhysicalDrive4 \\.\PhysicalDrive5 \\.\PhysicalDrive6
"#;

struct Args {
    level: RaidLevel,
    chunk_size: usize,
    stripes: u64,
    dataset: Option<String>,
    mountpoint: Option<String>,
    disks: Vec<String>,
}

fn parse_level(s: &str) -> Result<RaidLevel, String> {
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

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut level: Option<RaidLevel> = None;
    let mut chunk_size: Option<usize> = None;
    let mut stripes: Option<u64> = None;
    let mut dataset: Option<String> = None;
    let mut mountpoint: Option<String> = None;
    let mut disks: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--level" => {
                level = Some(parse_level(raw.get(i + 1).ok_or("--levelには値が必要です")?)?);
                i += 2;
            }
            "--chunk-size" => {
                chunk_size =
                    Some(raw.get(i + 1).ok_or("--chunk-sizeには値が必要です")?.parse().map_err(|_| "chunk-sizeが不正です")?);
                i += 2;
            }
            "--stripes" => {
                stripes = Some(raw.get(i + 1).ok_or("--stripesには値が必要です")?.parse().map_err(|_| "stripesが不正です")?);
                i += 2;
            }
            "--dataset" => {
                dataset = Some(raw.get(i + 1).ok_or("--datasetには値が必要です")?.clone());
                i += 2;
            }
            "--mountpoint" => {
                mountpoint = Some(raw.get(i + 1).ok_or("--mountpointには値が必要です")?.clone());
                i += 2;
            }
            other => {
                disks.push(other.to_string());
                i += 1;
            }
        }
    }

    let level = level.ok_or("--levelは必須です(help参照)")?;
    let chunk_size = chunk_size.ok_or("--chunk-sizeは必須です(help参照)")?;
    let stripes = stripes.ok_or("--stripesは必須です(help参照)")?;
    if disks.is_empty() {
        return Err("ディスクを最低1台指定してください(help参照)".to_string());
    }

    Ok(Args { level, chunk_size, stripes, dataset, mountpoint, disks })
}

fn open_devices(disks: &[String]) -> Result<Vec<FileBackedDevice>, String> {
    disks.iter().map(|p| FileBackedDevice::open(p).map_err(|e| format!("'{p}'を開けませんでした: {e}"))).collect()
}

/// `create`はOS非依存(WinFsp/FUSEどちらも不要)なので、featureに関わらず常に使える。
fn run_create(args: Args) -> Result<(), String> {
    let dataset = args.dataset.ok_or("createには--datasetが必須です")?;
    let devices = open_devices(&args.disks)?;
    let vdev = RaidZVdev::new(devices, args.level, args.chunk_size);
    let mut pool = Pool::new(vdev, args.stripes);
    pool.create_dataset(&dataset).map_err(|e| format!("データセット作成に失敗: {e}"))?;
    pool.save().map_err(|e| format!("メタデータの保存に失敗: {e}"))?;
    println!("プールを新規作成し、データセット'{dataset}'を作成・保存しました。");
    Ok(())
}

#[cfg(all(target_os = "linux", feature = "fuse_backend"))]
fn run_mount(args: Args) -> Result<(), String> {
    use open_raid_z_core::fuse_mount::mount_pool;
    let mountpoint = args.mountpoint.ok_or("mountには--mountpointが必須です")?;
    let devices = open_devices(&args.disks)?;
    let vdev = RaidZVdev::new(devices, args.level, args.chunk_size);
    let pool = Pool::open(vdev, args.stripes)
        .map_err(|e| format!("プールを開けませんでした(保存済みメタデータが無いか、パラメータが保存時と異なります): {e}"))?;
    let session = mount_pool(pool, &mountpoint).map_err(|e| format!("マウントに失敗しました: {e}"))?;
    println!("'{mountpoint}'へマウントしました。別のターミナルから`fusermount3 -u {mountpoint}`するとアンマウントされます。");
    session.join().map_err(|e| format!("マウントセッションの終了処理に失敗: {e}"))?;
    Ok(())
}

#[cfg(all(target_os = "windows", feature = "winfsp_backend"))]
fn run_mount(args: Args) -> Result<(), String> {
    use open_raid_z_core::mount::mount_pool;
    let mountpoint = args.mountpoint.ok_or("mountには--mountpointが必須です(例: \"Z:\")")?;
    let devices = open_devices(&args.disks)?;
    let vdev = RaidZVdev::new(devices, args.level, args.chunk_size);
    let pool = Pool::open(vdev, args.stripes)
        .map_err(|e| format!("プールを開けませんでした(保存済みメタデータが無いか、パラメータが保存時と異なります): {e}"))?;
    let mut host = mount_pool(pool, &mountpoint).map_err(|e| format!("マウントに失敗しました: {e:?}"))?;
    println!("'{mountpoint}'へマウントしました。Enterキーを押すとアンマウントされます。");
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
    host.unmount();
    Ok(())
}

#[cfg(not(any(
    all(target_os = "linux", feature = "fuse_backend"),
    all(target_os = "windows", feature = "winfsp_backend")
)))]
fn run_mount(_args: Args) -> Result<(), String> {
    Err(
        "このビルドには実マウント機能が含まれていません(Linuxでは`fuse_backend`、\
        Windowsでは`winfsp_backend` featureを有効にしてビルドしてください)。\
        `create`(プール作成)自体はこのビルドでも使えます。"
            .to_string(),
    )
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let Some(subcommand) = raw.first() else {
        eprintln!("{HELP}");
        std::process::exit(1);
    };

    let result = match subcommand.as_str() {
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        "create" => parse_args(&raw[1..]).and_then(run_create),
        "mount" => parse_args(&raw[1..]).and_then(run_mount),
        other => Err(format!("未知のサブコマンドです: '{other}'\n\n{HELP}")),
    };

    if let Err(msg) = result {
        eprintln!("{msg}");
        std::process::exit(1);
    }
}
