use dns_lookup::lookup_addr;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, Window};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::{self, JoinHandle};
use tokio::time::sleep;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt; // for creation_flags to hide child console windows

struct TraceState {
    cancel: AtomicBool,
    ping_handle: Mutex<Option<JoinHandle<()>>>,
    traceroute_pid: Mutex<Option<u32>>,
}

static TRACE_STATE: Lazy<TraceState> = Lazy::new(|| TraceState {
    cancel: AtomicBool::new(false),
    ping_handle: Mutex::new(None),
    traceroute_pid: Mutex::new(None),
});

fn set_traceroute_pid(pid: Option<u32>) {
    let mut guard = TRACE_STATE.traceroute_pid.lock().unwrap();
    *guard = pid;
}

fn abort_ping_loop() {
    if let Some(handle) = TRACE_STATE.ping_handle.lock().unwrap().take() {
        handle.abort();
    }
}

fn kill_traceroute_process() {
    if let Some(pid) = TRACE_STATE.traceroute_pid.lock().unwrap().take() {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"]) // kill tree force
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
    }
}

fn cancel_current() {
    TRACE_STATE.cancel.store(true, Ordering::SeqCst);
    abort_ping_loop();
    kill_traceroute_process();
}

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
    // Cancel any existing session first
    cancel_current();
    TRACE_STATE.cancel.store(false, Ordering::SeqCst);
    set_traceroute_pid(None);
    let win = window.clone();

    // Channel to receive hop updates as they are discovered
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HopInfo>();

    // Emit hop list incrementally as updates arrive, improving perceived latency
    let win_progress = window.clone();
    task::spawn(async move {
        let mut acc: Vec<HopInfo> = Vec::new();
        while let Some(h) = rx.recv().await {
            acc.push(h);
            let _ = win_progress.emit("hop_list_updated", &acc);
        }
    });

    // Perform traceroute in a blocking task to avoid blocking the async runtime
    let hops_res = task::spawn_blocking(move || perform_traceroute_blocking(&host, Some(tx))).await;

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

    // If cancelled while traceroute was running, bail out now
    if TRACE_STATE.cancel.load(Ordering::SeqCst) {
        eprintln!("start_trace: cancelled before final emit; exiting");
        return;
    }

    // Emit full hop list once (final state)
    eprintln!("emitting hop_list_updated with {} hops", hops.len());
    for h in &hops {
        eprintln!("  hop {} -> {} ({})", h.hop, h.ip, h.hostname);
    }
    let _ = window.emit("hop_list_updated", &hops);

    // Start continuous ping loop
    let window_clone = window.clone();
    let ping_handle = task::spawn(async move {
        eprintln!("starting continuous ping loop over {} hops", hops.len());
        loop {
            if TRACE_STATE.cancel.load(Ordering::SeqCst) {
                eprintln!("ping loop: cancel detected; exiting");
                break;
            }
            let mut handles = Vec::new();
            for hop in &hops {
                let hop = hop.clone();
                let win2 = window_clone.clone();
                // Spawn each ping as a separate task
                handles.push(task::spawn(async move {
                    let ip_res = hop.ip.parse::<IpAddr>();
                    match ip_res {
                        Ok(addr) => {
                            match ping_once_latency(addr, Duration::from_millis(900)).await {
                                Ok(Some(ms)) => {
                                    let data = PingData { ip: hop.ip.clone(), latency: Some(ms as u128), status: "ok".into() };
                                    // eprintln!("ping {}: ok {}ms", hop.ip, ms);
                                    let _ = win2.emit("new_ping_data", &data);
                                }
                                Ok(None) => {
                                    let data = PingData { ip: hop.ip.clone(), latency: None, status: "timeout".into() };
                                    // eprintln!("ping {}: timeout", hop.ip);
                                    let _ = win2.emit("new_ping_data", &data);
                                }
                                Err(e) => {
                                    eprintln!("ping {} error: {}", hop.ip, e);
                                    let data = PingData { ip: hop.ip.clone(), latency: None, status: "error".into() };
                                    let _ = win2.emit("new_ping_data", &data);
                                }
                            }
                        }
                        Err(_) => {
                            eprintln!("invalid ip for hop: {}", hop.ip);
                            let _ = win2.emit("new_ping_data", &PingData { ip: hop.ip.clone(), latency: None, status: "invalid_ip".into() });
                        }
                    }
                }));
            }
            
            // Wait for all pings to complete before sleeping for the next round
            for h in handles {
                let _ = h.await;
            }
            
            sleep(Duration::from_secs(1)).await;
        }
    });
    // store handle for later abort
    *TRACE_STATE.ping_handle.lock().unwrap() = Some(ping_handle);
}

fn perform_traceroute_blocking(host: &str, progress: Option<UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    // Always use system traceroute/tracert so we can kill it on demand
    do_traceroute_system(host, progress.as_ref())
}

#[cfg(not(target_os = "windows"))]
fn do_traceroute_crate(host: &str, progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    let mut hop_list: Vec<HopInfo> = Vec::new();
    // traceroute crate expects a socket address; pass port 0 per crate example
    let iter = traceroute::execute((host, 0)).map_err(|e| format!("{e}"))?;
    for hop_res in iter {
        let hop = hop_res.map_err(|e| format!("{e}"))?;
        let ip = hop.host.ip().to_string();
        let ttl = hop.ttl as u32;
        let hostname = reverse_dns(&ip);
        let info = HopInfo { hop: ttl, ip, hostname };
        if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
        hop_list.push(info);
    }
    eprintln!("do_traceroute_crate: collected {} hops", hop_list.len());
    Ok(hop_list)
}

#[cfg(target_os = "windows")]
fn do_traceroute_crate(_host: &str, _progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    Err("traceroute crate not supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
fn do_traceroute_system(host: &str, progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::collections::HashSet;
    // Use Windows built-in `tracert`. Flags:
    // -d: do not resolve names (faster, we do reverse DNS ourselves)
    // -h 30: max hops
    // -w 1000: timeout per hop in ms
    let mut cmd = Command::new("tracert");
    cmd.arg("-d").arg("-h").arg("30").arg("-w").arg("1000").arg(host)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW to avoid flashing consoles
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn tracert: {e}"))?;

    set_traceroute_pid(Some(child.id()));

    let stdout = child.stdout.take().ok_or_else(|| "no stdout from tracert".to_string())?;
    let reader = BufReader::new(stdout);

    let mut hop_list: Vec<HopInfo> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for line_res in reader.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => continue };
        let line_trim = line.trim().to_string();
        if line_trim.is_empty() { continue; }
        if line_trim.starts_with("Tracing route to") || line_trim.starts_with("over a maximum") { continue; }

        let parts = line_trim.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() { continue; }
        let ttl: u32 = match parts[0].parse() { Ok(v) => v, Err(_) => continue };
        if line_trim.contains("Request timed out") { continue; }

        if let Some(&last) = parts.last() {
            if last.parse::<IpAddr>().is_ok() {
                let ip_s = last.to_string();
                // Deduplicate consecutive or repeated hops (sometimes tracert prints repeats under packet loss)
                if seen.insert(format!("{}:{}", ttl, ip_s)) {
                    let hostname = reverse_dns(&ip_s);
                    let info = HopInfo { hop: ttl, ip: ip_s, hostname };
                    if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                    hop_list.push(info);
                }
            }
        }
    }

    let status = child.wait().map_err(|e| format!("wait tracert: {e}"))?;
    set_traceroute_pid(None);
    if !status.success() {
        eprintln!("tracert non-zero exit: {:?}", status.code());
        // Continue returning whatever hops we parsed so far, unless empty
        if hop_list.is_empty() {
            return Err(format!("tracert exit: {:?}", status.code()));
        }
    }

    if hop_list.is_empty() {
        eprintln!("tracert parser produced 0 hops; attempting last-resort dest resolution...");
        if let Ok(iter) = (host, 0).to_socket_addrs() {
            if let Some(sock) = iter.into_iter().next() {
                let ip = sock.ip().to_string();
                let hostname = reverse_dns(&ip);
                let info = HopInfo { hop: 1, ip, hostname };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
            }
        }
    }
    eprintln!("do_traceroute_system (windows): collected {} hops", hop_list.len());
    Ok(hop_list)
}

#[cfg(not(target_os = "windows"))]
fn do_traceroute_system(host: &str, progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut child = Command::new("traceroute")
        .arg("-n") // numeric output for speed; we'll reverse DNS ourselves
        .arg("-q").arg("1") // one probe per hop for responsiveness
        .arg("-w").arg("1") // timeout per probe seconds
        .arg("-m").arg("30") // max hops
        .arg(host)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn traceroute: {e}"))?;

    set_traceroute_pid(Some(child.id()));

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "no stdout from traceroute".to_string())?;
    let reader = BufReader::new(stdout);

    let mut hop_list: Vec<HopInfo> = Vec::new();
    for line_res in reader.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => continue };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        if line.starts_with("traceroute ") { continue; }

        let mut parts = line.split_whitespace();
        let ttl_s = parts.next().unwrap_or("");
        let ttl: u32 = match ttl_s.parse() { Ok(v) => v, Err(_) => continue };
        if let Some(ip_s) = parts.next() {
            if ip_s == "*" { continue; }
            if ip_s.parse::<IpAddr>().is_ok() {
                let hostname = reverse_dns(ip_s);
                let info = HopInfo { hop: ttl, ip: ip_s.to_string(), hostname };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
            }
        }
        if TRACE_STATE.cancel.load(Ordering::SeqCst) {
            eprintln!("traceroute reader: cancel detected; breaking");
            break;
        }
    }

    let _ = child.kill(); // ensure process exits if we bailed early
    let _ = child.wait();
    set_traceroute_pid(None);

    if hop_list.is_empty() {
        eprintln!("traceroute produced 0 hops; attempting last-resort dest resolution...");
        if let Ok(iter) = (host, 0).to_socket_addrs() {
            if let Some(sock) = iter.into_iter().next() {
                let ip = sock.ip().to_string();
                let hostname = reverse_dns(&ip);
                let info = HopInfo { hop: 1, ip, hostname };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
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

async fn ping_once_latency(addr: IpAddr, timeout: Duration) -> Result<Option<u64>, String> {
    use tokio::process::Command;
    let ip = addr.to_string();
    let start_overall = Instant::now();
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("ping");
        c.arg("-n").arg("1").arg("-w").arg(format!("{}", timeout.as_millis())).arg(&ip);
        #[cfg(target_os = "windows")]
        {
            c.creation_flags(0x08000000); // hide console window
        }
        c
    } else {
        let mut c = Command::new("ping");
        c.arg("-c").arg("1").arg("-W").arg(format!("{}", timeout.as_secs())).arg(&ip);
        c
    };

    let out = cmd.output().await.map_err(|e| format!("ping spawn: {e}"))?;
    if !out.status.success() {
        return Ok(None); // treat non-success as timeout/error
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    // Prefer robust parsing keyed off 'time=' or 'time<' to avoid matching TTL=64.
    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if let Some(idx) = lower.find("time<") {
            // Treat anything like 'time<1ms' as 1ms
            let after = &lower[idx + 5..];
            if after.trim_start().starts_with("1") { return Ok(Some(1)); }
            return Ok(Some(1));
        }
        if let Some(idx) = lower.find("time=") {
            let mut num = String::new();
            for ch in lower[idx + 5..].chars() {
                if ch.is_ascii_digit() || ch == '.' { num.push(ch); } else { break; }
            }
            if !num.is_empty() {
                if let Ok(val_f) = num.parse::<f64>() { return Ok(Some(val_f.round() as u64)); }
            }
        }
    }
    // Fallback: use process duration if we couldn't parse but exit was success
    let elapsed_ms = start_overall.elapsed().as_millis();
    Ok(Some(elapsed_ms as u64))
}

#[tauri::command]
pub async fn stop_trace() {
    eprintln!("stop_trace called");
    cancel_current();
}
