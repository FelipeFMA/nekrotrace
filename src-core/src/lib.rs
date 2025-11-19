use async_trait::async_trait;
use dns_lookup::lookup_addr;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::{self, JoinHandle};
use tokio::time::sleep;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize)]
pub struct HopInfo {
    pub hop: u32,
    pub ip: String,
    pub hostname: String,
    pub initial_latency: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingData {
    pub ip: String,
    pub latency: Option<u128>,
    pub status: String,
    pub seq: usize,
}

#[async_trait]
pub trait TraceEmitter: Send + Sync + 'static {
    async fn emit_hop_list(&self, payload: &Vec<HopInfo>);
    async fn emit_ping_data(&self, payload: &PingData);
}

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
                .args(["/PID", &pid.to_string(), "/T", "/F"])
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

pub async fn start_trace(host: String, emitter: Arc<dyn TraceEmitter>) {
    eprintln!("start_trace called with host: {}", host);
    cancel_current();
    TRACE_STATE.cancel.store(false, Ordering::SeqCst);
    set_traceroute_pid(None);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HopInfo>();

    let emitter_progress = emitter.clone();
    task::spawn(async move {
        let mut acc: Vec<HopInfo> = Vec::new();
        while let Some(h) = rx.recv().await {
            acc.push(h);
            emitter_progress.emit_hop_list(&acc).await;
        }
    });

    let hops_res = task::spawn_blocking(move || perform_traceroute_blocking(&host, Some(tx))).await;

    let hops = match hops_res {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            eprintln!("traceroute error: {e}");
            emitter.emit_hop_list(&Vec::new()).await;
            return;
        }
        Err(e) => {
            eprintln!("join error: {e}");
            emitter.emit_hop_list(&Vec::new()).await;
            return;
        }
    };

    if TRACE_STATE.cancel.load(Ordering::SeqCst) {
        eprintln!("start_trace: cancelled before final emit; exiting");
        return;
    }

    eprintln!("emitting hop_list_updated with {} hops", hops.len());
    emitter.emit_hop_list(&hops).await;

    let emitter_clone = emitter.clone();
    let ping_handle = task::spawn(async move {
        eprintln!("starting continuous ping loop over {} hops", hops.len());
        let mut seq = 0;
        loop {
            seq += 1;
            if TRACE_STATE.cancel.load(Ordering::SeqCst) {
                eprintln!("ping loop: cancel detected; exiting");
                break;
            }
            let mut handles = Vec::new();
            for hop in &hops {
                let hop = hop.clone();
                let emitter2 = emitter_clone.clone();
                let current_seq = seq;
                handles.push(task::spawn(async move {
                    let ip_res = hop.ip.parse::<IpAddr>();
                    match ip_res {
                        Ok(addr) => {
                            match ping_once_latency(addr, Duration::from_millis(900)).await {
                                Ok(Some(ms)) => {
                                    let data = PingData { ip: hop.ip.clone(), latency: Some(ms as u128), status: "ok".into(), seq: current_seq };
                                    emitter2.emit_ping_data(&data).await;
                                }
                                Ok(None) => {
                                    let data = PingData { ip: hop.ip.clone(), latency: None, status: "timeout".into(), seq: current_seq };
                                    emitter2.emit_ping_data(&data).await;
                                }
                                Err(e) => {
                                    eprintln!("ping {} error: {}", hop.ip, e);
                                    let data = PingData { ip: hop.ip.clone(), latency: None, status: "error".into(), seq: current_seq };
                                    emitter2.emit_ping_data(&data).await;
                                }
                            }
                        }
                        Err(_) => {
                            eprintln!("invalid ip for hop: {}", hop.ip);
                            let data = PingData { ip: hop.ip.clone(), latency: None, status: "invalid_ip".into(), seq: current_seq };
                            emitter2.emit_ping_data(&data).await;
                        }
                    }
                }));
            }
            
            for h in handles {
                let _ = h.await;
            }
            
            sleep(Duration::from_secs(1)).await;
        }
    });
    *TRACE_STATE.ping_handle.lock().unwrap() = Some(ping_handle);
}

pub fn stop_trace() {
    eprintln!("stop_trace called");
    cancel_current();
}

fn perform_traceroute_blocking(host: &str, progress: Option<UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    do_traceroute_system(host, progress.as_ref())
}

#[cfg(not(target_os = "windows"))]
fn do_traceroute_system(host: &str, progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut child = Command::new("traceroute")
        .arg("-n")
        .arg("-q").arg("1")
        .arg("-w").arg("1")
        .arg("-m").arg("30")
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
                let mut initial_latency = None;
                if let Some(lat_s) = parts.next() {
                    if let Ok(ms_f) = lat_s.parse::<f64>() {
                        initial_latency = Some(ms_f.round() as u64);
                    }
                }
                let info = HopInfo { hop: ttl, ip: ip_s.to_string(), hostname, initial_latency };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
            }
        }
        if TRACE_STATE.cancel.load(Ordering::SeqCst) {
            eprintln!("traceroute reader: cancel detected; breaking");
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    set_traceroute_pid(None);

    if hop_list.is_empty() {
        eprintln!("traceroute produced 0 hops; attempting last-resort dest resolution...");
        if let Ok(iter) = (host, 0).to_socket_addrs() {
            if let Some(sock) = iter.into_iter().next() {
                let ip = sock.ip().to_string();
                let hostname = reverse_dns(&ip);
                let info = HopInfo { hop: 1, ip, hostname, initial_latency: None };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
            }
        }
    }

    eprintln!("do_traceroute_system: collected {} hops", hop_list.len());
    Ok(hop_list)
}

#[cfg(target_os = "windows")]
fn do_traceroute_system(host: &str, progress: Option<&UnboundedSender<HopInfo>>) -> Result<Vec<HopInfo>, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::collections::HashSet;

    let mut cmd = Command::new("tracert");
    cmd.arg("-d").arg("-h").arg("30").arg("-w").arg("1000").arg(host)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.creation_flags(0x08000000);
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
                if seen.insert(format!("{}:{}", ttl, ip_s)) {
                    let hostname = reverse_dns(&ip_s);
                    let mut initial_latency = None;
                    if parts.len() > 1 {
                        let lat_str = parts[1].replace("<", "");
                        if let Ok(ms) = lat_str.parse::<u64>() {
                            initial_latency = Some(ms);
                        }
                    }
                    let info = HopInfo { hop: ttl, ip: ip_s, hostname, initial_latency };
                    if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                    hop_list.push(info);
                }
            }
        }
    }

    let status = child.wait().map_err(|e| format!("wait tracert: {e}"))?;
    set_traceroute_pid(None);
    if !status.success() {
        if hop_list.is_empty() {
            return Err(format!("tracert exit: {:?}", status.code()));
        }
    }

    if hop_list.is_empty() {
        if let Ok(iter) = (host, 0).to_socket_addrs() {
            if let Some(sock) = iter.into_iter().next() {
                let ip = sock.ip().to_string();
                let hostname = reverse_dns(&ip);
                let info = HopInfo { hop: 1, ip, hostname, initial_latency: None };
                if let Some(tx) = &progress { let _ = tx.send(info.clone()); }
                hop_list.push(info);
            }
        }
    }
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
            c.creation_flags(0x08000000);
        }
        c
    } else {
        let mut c = Command::new("ping");
        let secs = timeout.as_secs_f64();
        c.arg("-c").arg("1").arg("-W").arg(format!("{:.3}", secs)).arg(&ip);
        c
    };

    let out = cmd.output().await.map_err(|e| format!("ping spawn: {e}"))?;
    if !out.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if let Some(idx) = lower.find("ms") {
            let bytes = lower.as_bytes();
            let mut end_num = idx;
            while end_num > 0 && bytes[end_num - 1] == b' ' {
                end_num -= 1;
            }
            let mut start = end_num;
            while start > 0 {
                let b = bytes[start - 1];
                if b.is_ascii_digit() || b == b'.' {
                    start -= 1;
                } else {
                    break;
                }
            }
            if start < end_num {
                let num_str = &lower[start..end_num];
                if let Ok(val_f) = num_str.parse::<f64>() {
                    return Ok(Some(val_f.round() as u64));
                }
            }
        }
    }
    let elapsed_ms = start_overall.elapsed().as_millis();
    Ok(Some(elapsed_ms as u64))
}
