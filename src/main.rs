use clap::Parser;
use libc::{self, iovec};
use std::ffi::CString;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[derive(Parser)]
#[clap(
    name    = "rust-run-jail",
    author  = "SoumaSakaguchi",
    version = "v0.1.0",
    about   = "Rust製FreeBSD Jailランタイム"
)]

struct AppArg {
    #[clap(value_parser, required = true)]
    command: Vec<String>,
}

fn main() {
    let arg: AppArg = AppArg::parse();
    let (cmd, cmd_args) = parse_cmd_and_args(arg.command);

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
    jailremove_syscall(jid);

    println!("Succsess!")
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
    let jid = jailset_syscall();
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

fn jailset_syscall() -> i32 {
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
