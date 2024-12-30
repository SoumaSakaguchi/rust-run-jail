use std::env;
use std::ffi::CString;
use std::process::Command;
use std::thread;
use libc::{self, iovec};

fn main() {
	let (cmd, options) = get_args();
	let handle = thread::spawn(move || {
		child_process(&cmd);
	});
	println!("Options: {:?}", options);

	handle.join().expect("スレッドの実行に失敗");
	
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

fn child_process(cmd: &str) {
	make_jail();
	let output = Command::new(cmd)
	  .output()
	  .expect("コマンドの実行に失敗");

	if output.stdout.len() != 0 {
		println!("{}", String::from_utf8_lossy(&output.stdout));
	}
	if output.stderr.len() != 0 {
		eprintln!("{}", String::from_utf8_lossy(&output.stderr));
	}
}

fn make_jail() {
	let keys_and_values = vec![
		(CString::new("path").unwrap(), CString::new("/").unwrap().into_bytes_with_nul()),
		(CString::new("vnet").unwrap(), 1u32.to_ne_bytes().to_vec()),
		(CString::new("children.max").unwrap(), 99u32.to_ne_bytes().to_vec()),
		(CString::new("persist").unwrap(), Vec::new()), // 値なし (nil 相当)
	];

	// iovec配列を作成
	let mut iov = Vec::new();
	for (key, value) in &keys_and_values {
		iov.push(iovec {
			iov_base: key.as_ptr() as *mut libc::c_void,
			iov_len: key.as_bytes_with_nul().len(),
		});
		iov.push(iovec {
			iov_base: if value.is_empty() {
				std::ptr::null_mut() // 値がない場合
			} else {
				value.as_ptr() as *mut libc::c_void
			},
			iov_len: value.len(),
		});
	}

	// `jail_set`呼び出し
	let flags = libc::JAIL_CREATE; // フラグを指定
	let result = unsafe { libc::jail_set(iov.as_ptr() as *mut iovec, iov.len() as u32, flags) };

	// 結果の確認
	if result < 0 {
		eprintln!("jail_set failed: {}", std::io::Error::last_os_error());
	} else {
		println!("Jail created successfully. jid: {}", result);
	}
}

