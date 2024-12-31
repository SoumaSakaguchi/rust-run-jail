use libc::{self, iovec};
use std::env;
use std::ffi::CString;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

fn main() {
    let (cmd, options) = get_args();

    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn({
        let cmd = cmd.clone();
        let options = options.clone();
        move || {
            child_process(cmd, options, sender);
        }
    });

    println!("Options: {:?}", options);
    let jid = receiver.recv().expect("Jail IDの受信に失敗");
    println!("jid: {}", jid);

    handle.join().expect("スレッドの実行に失敗");
    kill_jail(jid);

    println!("Succsess!")
}

fn get_args() -> (String, Vec<String>) {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [options...]", args[0]);
        std::process::exit(1);
    }
    let cmd = args[1].clone();
    let options = args[2..].to_vec();
    (cmd, options)
}

fn child_process(cmd: String, options: Vec<String>, sender: mpsc::Sender<i32>) {
    let jid = make_jail();
    println!("Created jail with ID: {}", jid);
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

fn make_jail() -> i32 {
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

fn kill_jail(jid: i32) {
    let _result = unsafe { libc::jail_remove(jid) };
}
