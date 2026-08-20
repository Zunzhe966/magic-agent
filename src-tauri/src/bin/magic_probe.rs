use magic_agent_lib::{start_proxy_standalone, stop_proxy_standalone};
fn main() {
    match start_proxy_standalone() {
        Ok(s) => {
            println!("STARTED port={} node={:?}", s.port, s.node);
            std::thread::sleep(std::time::Duration::from_secs(8));
            let _ = stop_proxy_standalone();
            println!("STOPPED");
        }
        Err(e) => {
            eprintln!("ERROR {e}");
            std::process::exit(1);
        }
    }
}
