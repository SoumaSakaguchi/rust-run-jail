use clap::{Parser, Subcommand};
use libc::iovec;
use std::fs;
use std::ffi::CString;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[derive(Parser)]
#[clap(
    name = "rust-run-jail",
    author = "SoumaSakaguchi",
    version = "v0.1.0",
    about = "Rust製FreeBSD Jailランタイム"
)]
struct AppArg {
    #[clap(subcommand)]
    subcmd: SubCmd,
}

#[derive(Subcommand)]
enum SubCmd {
    Run {
        #[clap(short, long, help = "configのパス", required = true)]
        path: String,

        #[clap(value_parser, required = true)]
        command: Vec<String>,

        #[clap(short, long, help = "実行後にJailを破棄する")]
        destroy: bool,
    },

    List {
        #[clap(short, long)]
        all: bool,
    },

    Template {
        #[clap(value_parser, help = "テンプレートを指定（netns, freebsd, linux）", required = true)]
        tmp: String,
    },
}

fn main() {
    let args: AppArg = AppArg::parse();

    match args.subcmd {
        SubCmd::Run { path, command, destroy } => {
            run_jail(path, command, destroy);
        }
        SubCmd::List { all } => {
            show_list(all);
        }
        SubCmd::Template { tmp } => {
            if tmp == "netns" {
                create_netns();
            } else if tmp == "freebsd" {
                create_freebsd();
            } else if tmp == "linux" {
                create_linux();
            } else {
                println!("選択されたテンプレートは未実装です: {}", tmp);
            }
        }
    }
}

fn run_jail(path: String, command: Vec<String>, destroy: bool) {
    println!("Config Path: {}", path);

    let (cmd, cmd_args) = parse_cmd_and_args(command);

    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn({
        let cmd = cmd.clone();
        let cmd_args = cmd_args.clone();
        move || {
            child_process(cmd, cmd_args, sender);
        }
    });

    let jid = receiver.recv().expect("Jail IDの受信に失敗");
    println!("jid: {}", jid);

    handle.join().expect("スレッドの実行に失敗");

    if destroy {
        jailremove_syscall(jid);
    }

    println!("Success!");
}

fn parse_cmd_and_args(command: Vec<String>) -> (String, Vec<String>) {
    if command.is_empty() {
        eprintln!("コマンドが指定されていません");
        std::process::exit(1);
    }

    let cmd = command[0].clone();
    let cmd_args = command[1..].to_vec();
    (cmd, cmd_args)
}

fn child_process(cmd: String, options: Vec<String>, sender: mpsc::Sender<i32>) {
    let keys_and_values = vec![];
    let jid = jailset_syscall(keys_and_values);
    sender.send(jid).expect("Jail IDの送信に失敗");

    let mut command = Command::new(cmd);
    command.args(&options);

    let status = command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .expect("コマンドの実行に失敗");

    if !status.success() {
        eprintln!("コマンドがエラー終了: {}", status);
    }
}

fn jailset_syscall(keys_and_values: Vec<(CString, Vec<u8>)>) -> i32 {
    let mut iov = Vec::new();
    for (key, value) in &keys_and_values {
        iov.push(iovec {
            iov_base: key.as_ptr() as *mut libc::c_void,
            iov_len: key.as_bytes_with_nul().len(),
        });
        iov.push(iovec {
            iov_base: if value.is_empty() {
                std::ptr::null_mut()
            } else {
                value.as_ptr() as *mut libc::c_void
            },
            iov_len: value.len(),
        });
    }

    let flags = libc::JAIL_CREATE;
    let result = unsafe { libc::jail_set(iov.as_ptr() as *mut iovec, iov.len() as u32, flags) };

    if result < 0 {
        eprintln!("jail_set failed: {}", std::io::Error::last_os_error());
        std::process::exit(1);
    }
    result
}

fn jailremove_syscall(jid: i32) {
    let _result = unsafe { libc::jail_remove(jid) };
}

fn show_list(all: bool) {
    let option = if all { "-n" } else { "-N" };

    let output = Command::new("jls")
        .arg(option)
        .output()
        .expect("コマンドの実行に失敗しました");

    if output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }
}

fn create_netns() {
    let keys_and_values = vec![
        (
            CString::new("path").unwrap(),
            CString::new("/").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("vnet").unwrap(),
            1u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("children.max").unwrap(),
            99u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("persist").unwrap(),
            Vec::new(),
        ),
    ];

    let jid = jailset_syscall(keys_and_values);
    println!("Success create netns compat jail! JID: {}.", jid);
}

fn create_freebsd() {
    let rootfs_path = "/tmp/jails/freebsd/rootfs";
    let tar_url = "https://download.freebsd.org/releases/amd64/14.2-RELEASE/base.txz";
    let tar_file_path = "/tmp/freebsd_base.txz";

    if let Err(e) = fs::create_dir_all(rootfs_path) {
        eprintln!("ディレクトリ作成に失敗しました: {}", e);
        return;
    }
    println!("ディレクトリ作成: {}", rootfs_path);

    let curl_status = Command::new("curl")
        .args(&["-o", tar_file_path, tar_url])
        .status();

    match curl_status {
        Ok(status) if status.success() => {
            println!("tarファイルをダウンロードしました: {}", tar_file_path);
        }
        _ => {
            eprintln!("tarファイルのダウンロードに失敗しました");
            return;
        }
    }

    let tar_status = Command::new("tar")
        .args(&["-xvf", tar_file_path, "-C", rootfs_path])
        .status();

    match tar_status {
        Ok(status) if status.success() => {
            println!("tarファイルを展開しました: {}", rootfs_path);
        }
        _ => {
            eprintln!("tarファイルの展開に失敗しました");
            return;
        }
    }

    if let Err(e) = fs::remove_file(tar_file_path) {
        eprintln!("ダウンロードしたtarファイルの削除に失敗しました: {}", e);
    } else {
        println!("tarファイルを削除しました: {}", tar_file_path);
    }
    let keys_and_values = vec![
        (
            CString::new("path").unwrap(),
            CString::new("/tmp/jails/freebsd/rootfs").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("name").unwrap(),
            CString::new("test-jail").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("host.hostname").unwrap(),
            CString::new("freebsd.org").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("vnet").unwrap(),
            1u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("allow.mount.procfs").unwrap(),
            Vec::new(),
        ),
        (
            CString::new("allow.mount.devfs").unwrap(),
            Vec::new(),
        ),
        (
            CString::new("children.max").unwrap(),
            99u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("persist").unwrap(),
            Vec::new(),
        ),
    ];

    let jid = jailset_syscall(keys_and_values);
    println!("Success create freebsd jail! JID: {}.", jid);
}

fn create_linux() {
    let rootfs_path = "/tmp/jails/linux/rootfs";
    let tar_url = "https://partner-images.canonical.com/core/jammy/current/ubuntu-jammy-core-cloudimg-amd64-root.tar.gz";
    let tar_file_path = "/tmp/linux_base.txz";

    if let Err(e) = fs::create_dir_all(rootfs_path) {
        eprintln!("ディレクトリ作成に失敗しました: {}", e);
        return;
    }
    println!("ディレクトリ作成: {}", rootfs_path);

    let curl_status = Command::new("curl")
        .args(&["-o", tar_file_path, tar_url])
        .status();

    match curl_status {
        Ok(status) if status.success() => {
            println!("tarファイルをダウンロードしました: {}", tar_file_path);
        }
        _ => {
            eprintln!("tarファイルのダウンロードに失敗しました");
            return;
        }
    }

    let tar_status = Command::new("tar")
        .args(&["-xvf", tar_file_path, "-C", rootfs_path])
        .status();

    match tar_status {
        Ok(status) if status.success() => {
            println!("tarファイルを展開しました: {}", rootfs_path);
        }
        _ => {
            eprintln!("tarファイルの展開に失敗しました");
            return;
        }
    }

    if let Err(e) = fs::remove_file(tar_file_path) {
        eprintln!("ダウンロードしたtarファイルの削除に失敗しました: {}", e);
    } else {
        println!("tarファイルを削除しました: {}", tar_file_path);
    }
    let keys_and_values = vec![
        (
            CString::new("path").unwrap(),
            CString::new("/tmp/jails/linux/rootfs").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("name").unwrap(),
            CString::new("linux-jail").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("host.hostname").unwrap(),
            CString::new("linux.org").unwrap().into_bytes_with_nul(),
        ),
        (
            CString::new("vnet").unwrap(),
            1u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("allow.mount.procfs").unwrap(),
            Vec::new(),
        ),
        (
            CString::new("allow.mount.devfs").unwrap(),
            Vec::new(),
        ),
        (
            CString::new("children.max").unwrap(),
            99u32.to_ne_bytes().to_vec(),
        ),
        (
            CString::new("persist").unwrap(),
            Vec::new(),
        ),
    ];

    let jid = jailset_syscall(keys_and_values);
    println!("Success create freebsd jail! JID: {}.", jid);
}

