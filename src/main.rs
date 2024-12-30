use std::env;
use std::process::Command;
use std::thread;

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

