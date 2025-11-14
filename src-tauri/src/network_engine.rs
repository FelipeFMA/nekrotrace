use dns_lookup::lookup_addr;
use serde::Serialize;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::{Duration, Instant};
use tauri::{Emitter, Window};
use tokio::task;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize)]
pub struct HopInfo {
    pub hop: u32,
    pub ip: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingData {
    pub ip: String,
    pub latency: Option<u128>,
    pub status: String,
}

#[tauri::command]
pub async fn start_trace(host: String, window: Window) {
    eprintln!("start_trace called with host: {}", host);
    let win = window.clone();

    // Perform traceroute in a blocking task to avoid blocking the async runtime
    let hops_res = task::spawn_blocking(move || perform_traceroute_blocking(&host)).await;

    let hops = match hops_res {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            eprintln!("traceroute error: {e}");
            let _ = win.emit("hop_list_updated", &Vec::<HopInfo>::new());
            return;
        }
        Err(e) => {
            eprintln!("join error: {e}");
            let _ = win.emit("hop_list_updated", &Vec::<HopInfo>::new());
            return;
        }
    };

    // Emit full hop list once
    eprintln!("emitting hop_list_updated with {} hops", hops.len());
    for h in &hops {
        eprintln!("  hop {} -> {} ({})", h.hop, h.ip, h.hostname);
    }
    let _ = window.emit("hop_list_updated", &hops);

    // Start continuous ping loop
    let window_clone = window.clone();
    task::spawn(async move {
        eprintln!("starting continuous ping loop over {} hops", hops.len());
        loop {
            for hop in &hops {
                let ip = hop.ip.parse::<IpAddr>();
                let win2 = window_clone.clone();

                match ip {
                    Ok(addr) => {
                        let start = Instant::now();
                        let status;
                        let latency;

                        match ping_once(addr, Duration::from_millis(900)).await {
                            Ok(true) => {
                                latency = Some(start.elapsed().as_millis());
                                status = "ok".to_string();
                            }
                            Ok(false) => {
                                latency = None;
                                status = "timeout".to_string();
                            }
                            Err(_) => {
                                latency = None;
                                status = "error".to_string();
                            }
                        }

                        let data = PingData {
                            ip: hop.ip.clone(),
                            latency,
                            status,
                        };
                        if let Some(ms) = data.latency { eprintln!("ping {}: {} {}ms", hop.ip, data.status, ms); } else { eprintln!("ping {}: {}", hop.ip, data.status); }
                        let _ = win2.emit("new_ping_data", &data);
                    }
                    Err(_) => {
                        eprintln!("invalid ip for hop: {}", hop.ip);
                        let _ = win2.emit(
                            "new_ping_data",
                            &PingData { ip: hop.ip.clone(), latency: None, status: "invalid_ip".into() },
                        );
                    }
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
    });
}

fn perform_traceroute_blocking(host: &str) -> Result<Vec<HopInfo>, String> {
    eprintln!("perform_traceroute_blocking: trying crate traceroute...");
    match do_traceroute_crate(host) {
        Ok(v) => Ok(v),
        Err(e) => {
            eprintln!("crate traceroute failed: {}", e);
            if e.contains("Operation not permitted") || e.contains("Permission") {
                eprintln!("falling back to system traceroute");
                do_traceroute_system(host)
            } else {
                Err(e)
            }
        }
    }
}

fn do_traceroute_crate(host: &str) -> Result<Vec<HopInfo>, String> {
    let mut hop_list: Vec<HopInfo> = Vec::new();
    // traceroute crate expects a socket address; pass port 0 per crate example
    let iter = traceroute::execute((host, 0)).map_err(|e| format!("{e}"))?;
    for hop_res in iter {
        let hop = hop_res.map_err(|e| format!("{e}"))?;
        let ip = hop.host.ip().to_string();
        let ttl = hop.ttl as u32;
        let hostname = reverse_dns(&ip);
        hop_list.push(HopInfo { hop: ttl, ip, hostname });
    }
    eprintln!("do_traceroute_crate: collected {} hops", hop_list.len());
    Ok(hop_list)
}

fn do_traceroute_system(host: &str) -> Result<Vec<HopInfo>, String> {
    use std::process::Command;
    let output = Command::new("traceroute")
        .arg("-n").arg("-q").arg("1").arg("-w").arg("1").arg("-m").arg("30")
        .arg(host)
        .output()
        .map_err(|e| format!("spawn traceroute: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("system traceroute non-zero exit (code {:?}): {}", output.status.code(), stderr);
        return Err(format!("traceroute exit: {:?}", output.status.code()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("system traceroute stdout (first 5 lines):");
    for (i, line) in stdout.lines().take(5).enumerate() { eprintln!("  {}: {}", i+1, line); }
    let mut hop_list: Vec<HopInfo> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        // Skip header line which starts with the destination
        if line.starts_with("traceroute ") { continue; }
        // Expected format: " 1  192.0.2.1  10.123 ms" or " 3  * * *"
        let mut parts = line.split_whitespace();
        let ttl_s = parts.next().unwrap_or("");
        let ttl: u32 = match ttl_s.parse() { Ok(v) => v, Err(_) => continue };
        if let Some(ip_s) = parts.next() {
            if ip_s == "*" { continue; }
            if ip_s.parse::<IpAddr>().is_ok() {
                let hostname = reverse_dns(ip_s);
                hop_list.push(HopInfo { hop: ttl, ip: ip_s.to_string(), hostname });
            }
        }
    }
    if hop_list.is_empty() {
        // Last-resort: include destination itself so UI has a target to ping
        eprintln!("parser produced 0 hops; attempting last-resort dest resolution...");
        if let Ok(iter) = (host, 0).to_socket_addrs() {
            if let Some(sock) = iter.into_iter().next() {
                let ip = sock.ip().to_string();
                let hostname = reverse_dns(&ip);
                hop_list.push(HopInfo { hop: 1, ip, hostname });
            }
        }
    }
    eprintln!("do_traceroute_system: collected {} hops", hop_list.len());
    Ok(hop_list)
}

fn reverse_dns(ip_str: &str) -> String {
    ip_str
        .parse::<IpAddr>()
        .ok()
        .and_then(|ip| lookup_addr(&ip).ok())
        .unwrap_or_else(|| ip_str.to_string())
}

async fn ping_once(addr: IpAddr, timeout: Duration) -> Result<bool, String> {
    // Portable fallback using system ping to avoid tokio-ping 0.3 runtime incompatibilities.
    // Success is based on process exit status; latency measured externally in caller.
    use tokio::process::Command;

    let ip = addr.to_string();
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("ping");
        c.arg("-n").arg("1").arg("-w").arg(format!("{}", timeout.as_millis())).arg(&ip);
        c
    } else {
        let mut c = Command::new("ping");
        // -c 1 one packet; -W timeout in seconds
        c.arg("-c").arg("1").arg("-W").arg(format!("{}", timeout.as_secs()))
            .arg(&ip);
        c
    };

    match cmd.output().await {
        Ok(out) => Ok(out.status.success()),
        Err(e) => {
            eprintln!("ping spawn error for {}: {}", ip, e);
            Err(format!("ping spawn: {e}"))
        }
    }
}
