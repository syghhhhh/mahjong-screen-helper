// Original diagnostic-runner fixture. This is never packaged as a rule probe.
fn main() {
    if let Ok(path) = std::env::var("MAHJONG_PROBE_CONTROL_MARKER") {
        std::fs::write(path, b"executed").unwrap();
    }
    match std::env::var("MAHJONG_PROBE_CONTROL_MODE").as_deref() {
        Ok("pass") => println!(
            "running 1 test\ntest synthetic_fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"
        ),
        Ok("fail") => {
            println!("synthetic 中文 failure output");
            eprintln!("synthetic 中文 failure error");
            std::process::exit(7);
        }
        Ok("timeout") => std::thread::sleep(std::time::Duration::from_secs(10)),
        _ => println!("running 0 tests\ntest result: ok. 0 passed; 0 failed;"),
    }
}
