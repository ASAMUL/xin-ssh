use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use russh::{client, ChannelMsg};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{timeout, Duration};

use crate::models::{
    RemoteCpuInfo, RemoteDiskInfo, RemoteMemoryInfo, RemoteNetworkInfo, RemoteProcessInfo,
    RemoteSystemInfoRequest, RemoteSystemSnapshot,
};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;
use crate::util::{connection_error, now_millis, shell_quote};

const SYSTEM_INFO_SECTION_MARKER: &str = "__XIN_SYSTEM_INFO__";

#[tauri::command]
pub async fn get_remote_system_info(
    registry: State<'_, TerminalRegistry>,
    request: RemoteSystemInfoRequest,
) -> Result<RemoteSystemSnapshot, String> {
    validate_system_info_request(&request)?;

    timeout(
        Duration::from_secs(8),
        registry.get_system_info(&request.session_id),
    )
    .await
    .map_err(|_| "System info request timed out after 8 seconds".to_string())?
}

struct RemoteCommandOutput {
    stdout: String,
    stderr: String,
    exit_status: Option<u32>,
}

pub(crate) async fn collect_system_info_on_session(
    session_id: &str,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
) -> Result<RemoteSystemSnapshot, String> {
    let output = timeout(
        Duration::from_secs(7),
        execute_remote_command(session, remote_system_info_command()),
    )
    .await
    .map_err(|_| "Remote system info command timed out after 7 seconds".to_string())??;

    if output.exit_status.unwrap_or(0) != 0 && output.stdout.trim().is_empty() {
        let detail = output.stderr.trim();
        if detail.is_empty() {
            return Err("Remote system info command failed".to_string());
        }

        return Err(detail.to_string());
    }

    Ok(parse_system_snapshot(session_id, &output.stdout))
}

async fn execute_remote_command(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    command: String,
) -> Result<RemoteCommandOutput, String> {
    let mut channel = {
        let session_handle = session.lock().await;
        session_handle
            .channel_open_session()
            .await
            .map_err(connection_error)?
    };

    channel.exec(true, command).await.map_err(connection_error)?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_status = None;

    loop {
        let Some(message) = channel.wait().await else {
            break;
        };

        match message {
            ChannelMsg::Data { data } => {
                stdout.extend_from_slice(&data);
            }
            ChannelMsg::ExtendedData { data, .. } => {
                stderr.extend_from_slice(&data);
            }
            ChannelMsg::ExitStatus { exit_status: status } => {
                exit_status = Some(status);
            }
            // Eof 之后可能还有 ExitStatus/Close，等通道真正关闭再退出
            ChannelMsg::Close => {
                break;
            }
            _ => {}
        }
    }

    Ok(RemoteCommandOutput {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_status,
    })
}

fn remote_system_info_command() -> String {
    let script = r#"PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH"
MARK="__XIN_SYSTEM_INFO__"
section() { printf '\n%s %s\n' "$MARK" "$1"; }

section HOST
hostname 2>/dev/null || true
uname -srmo 2>/dev/null || uname -a 2>/dev/null || true

section UPTIME
cat /proc/uptime 2>/dev/null || true

section LOAD
cat /proc/loadavg 2>/dev/null || true

section CPU1
grep '^cpu ' /proc/stat 2>/dev/null || true
sleep 0.25
section CPU2
grep '^cpu ' /proc/stat 2>/dev/null || true

section CPUINFO
awk -F: '/model name|Hardware|Processor/ {gsub(/^[ \t]+/,"",$2); print $2; exit}' /proc/cpuinfo 2>/dev/null || true
if command -v nproc >/dev/null 2>&1; then
  nproc 2>/dev/null || true
else
  grep -c '^processor' /proc/cpuinfo 2>/dev/null || true
fi

section MEM
cat /proc/meminfo 2>/dev/null || true

section NET
cat /proc/net/dev 2>/dev/null || true
section NET_ADDR
if command -v ip >/dev/null 2>&1; then
  ip -o -4 addr show scope global 2>/dev/null | awk '{split($2,n,"@"); split($4,a,"/"); print n[1], a[1]}' || true
fi

if df -P -B1 / >/dev/null 2>&1; then
  section DF_BYTES
  (df -P -B1 / /home /var 2>/dev/null || df -P -B1 / 2>/dev/null) | awk '!seen[$6]++'
else
  section DF_KIB
  (df -Pk / /home /var 2>/dev/null || df -Pk / 2>/dev/null) | awk '!seen[$6]++'
fi

section PS
{
  ps -eo pid=,user=,pcpu=,pmem=,rss=,comm=,args= 2>/dev/null | sort -k3 -nr | head -n 6
  ps -eo pid=,user=,pcpu=,pmem=,rss=,comm=,args= 2>/dev/null | sort -k4 -nr | head -n 6
} | awk '!seen[$1]++' | head -n 10 || true
"#;

    // 非 login shell：避免每次采样都 source 远端 profile（nvm/conda 等会显著拖慢），PATH 在脚本内自补
    format!("sh -c {}", shell_quote(script))
}

fn parse_system_snapshot(session_id: &str, output: &str) -> RemoteSystemSnapshot {
    let sections = split_system_sections(output);
    let meminfo = parse_meminfo(lines_for(&sections, "MEM"));
    let host_lines = non_empty_lines(lines_for(&sections, "HOST"));

    RemoteSystemSnapshot {
        session_id: session_id.to_string(),
        captured_at: now_millis(),
        hostname: host_lines.first().cloned(),
        kernel: host_lines.get(1).cloned(),
        uptime_seconds: first_non_empty(lines_for(&sections, "UPTIME"))
            .and_then(|line| parse_uptime_seconds(&line)),
        load_average: first_non_empty(lines_for(&sections, "LOAD"))
            .map(|line| parse_load_average(&line))
            .unwrap_or_default(),
        cpu: parse_cpu_info(&sections),
        memory: parse_memory_info(&meminfo),
        swap: parse_swap_info(&meminfo),
        networks: parse_networks(
            lines_for(&sections, "NET"),
            lines_for(&sections, "NET_ADDR"),
        ),
        processes: parse_processes(lines_for(&sections, "PS")),
        disks: parse_disks(&sections),
    }
}

fn split_system_sections(output: &str) -> HashMap<String, Vec<String>> {
    let mut sections = HashMap::new();
    let mut current_section: Option<String> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(name) = line.strip_prefix(SYSTEM_INFO_SECTION_MARKER) {
            let section_name = name.trim().to_string();
            sections.entry(section_name.clone()).or_insert_with(Vec::new);
            current_section = Some(section_name);
            continue;
        }

        if let Some(section_name) = &current_section {
            sections
                .entry(section_name.clone())
                .or_insert_with(Vec::new)
                .push(line.to_string());
        }
    }

    sections
}

fn lines_for<'a>(sections: &'a HashMap<String, Vec<String>>, name: &str) -> &'a [String] {
    sections.get(name).map(Vec::as_slice).unwrap_or(&[])
}

fn non_empty_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn first_non_empty(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn parse_cpu_info(sections: &HashMap<String, Vec<String>>) -> RemoteCpuInfo {
    let cpuinfo_lines = non_empty_lines(lines_for(sections, "CPUINFO"));
    let core_count = cpuinfo_lines
        .get(1)
        .and_then(|value| parse_u32(value))
        .filter(|value| *value > 0);

    RemoteCpuInfo {
        usage_percent: parse_cpu_usage(
            first_non_empty(lines_for(sections, "CPU1")).as_deref(),
            first_non_empty(lines_for(sections, "CPU2")).as_deref(),
        ),
        core_count,
        model_name: cpuinfo_lines.first().cloned(),
    }
}

fn parse_cpu_usage(first_line: Option<&str>, second_line: Option<&str>) -> Option<f64> {
    let (first_idle, first_total) = parse_cpu_stat(first_line?)?;
    let (second_idle, second_total) = parse_cpu_stat(second_line?)?;
    let total_delta = second_total.checked_sub(first_total)?;
    let idle_delta = second_idle.checked_sub(first_idle)?;

    if total_delta == 0 {
        return None;
    }

    Some(round_one_decimal(
        (total_delta.saturating_sub(idle_delta)) as f64 * 100.0 / total_delta as f64,
    ))
}

fn parse_cpu_stat(line: &str) -> Option<(u64, u64)> {
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(parse_u64)
        .collect();

    if values.len() < 4 {
        return None;
    }

    let idle = values
        .get(3)
        .copied()
        .unwrap_or(0)
        .saturating_add(values.get(4).copied().unwrap_or(0));
    let total = values.iter().copied().sum();

    Some((idle, total))
}

fn parse_meminfo(lines: &[String]) -> HashMap<String, u64> {
    let mut meminfo = HashMap::new();

    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Some(kib) = value.split_whitespace().next().and_then(parse_u64) else {
            continue;
        };

        meminfo.insert(key.trim().to_string(), kib.saturating_mul(1024));
    }

    meminfo
}

fn parse_memory_info(meminfo: &HashMap<String, u64>) -> RemoteMemoryInfo {
    let total = meminfo.get("MemTotal").copied();
    let available = meminfo.get("MemAvailable").copied().or_else(|| {
        Some(
            meminfo.get("MemFree").copied().unwrap_or(0)
                + meminfo.get("Buffers").copied().unwrap_or(0)
                + meminfo.get("Cached").copied().unwrap_or(0),
        )
    });
    memory_info_from_total_available(total, available)
}

fn parse_swap_info(meminfo: &HashMap<String, u64>) -> RemoteMemoryInfo {
    memory_info_from_total_available(
        meminfo.get("SwapTotal").copied(),
        meminfo.get("SwapFree").copied(),
    )
}

fn memory_info_from_total_available(
    total: Option<u64>,
    available: Option<u64>,
) -> RemoteMemoryInfo {
    let used = total
        .zip(available)
        .map(|(total, available)| total.saturating_sub(available));

    RemoteMemoryInfo {
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        usage_percent: used
            .zip(total)
            .and_then(|(used, total)| percent_from_parts(used, total)),
    }
}

fn parse_networks(lines: &[String], address_lines: &[String]) -> Vec<RemoteNetworkInfo> {
    let mut networks = Vec::new();
    let ipv4_addresses = parse_network_ipv4_addresses(address_lines);

    for line in lines {
        let Some((name, values)) = line.split_once(':') else {
            continue;
        };
        let network_name = name.trim().to_string();
        let values: Vec<u64> = values.split_whitespace().filter_map(parse_u64).collect();

        if values.len() < 10 {
            continue;
        }

        networks.push(RemoteNetworkInfo {
            ipv4_address: ipv4_addresses.get(&network_name).cloned(),
            name: network_name,
            rx_bytes: values[0],
            rx_packets: values[1],
            tx_bytes: values[8],
            tx_packets: values[9],
        });
    }

    let mut selected: Vec<RemoteNetworkInfo> = networks
        .iter()
        .filter(|network| network.name != "lo")
        .cloned()
        .collect();
    if selected.is_empty() {
        selected = networks;
    }

    selected.sort_by(|left, right| {
        right
            .rx_bytes
            .saturating_add(right.tx_bytes)
            .cmp(&left.rx_bytes.saturating_add(left.tx_bytes))
    });
    selected.truncate(4);
    selected
}

fn parse_network_ipv4_addresses(lines: &[String]) -> HashMap<String, String> {
    let mut addresses = HashMap::new();

    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        addresses
            .entry(parts[0].to_string())
            .or_insert_with(|| parts[1].to_string());
    }

    addresses
}

fn parse_processes(lines: &[String]) -> Vec<RemoteProcessInfo> {
    lines.iter().filter_map(|line| parse_process(line)).collect()
}

fn parse_process(line: &str) -> Option<RemoteProcessInfo> {
    let parts = split_whitespace_tail(line, 7);
    if parts.len() < 6 {
        return None;
    }

    let pid = parse_u32(&parts[0])?;
    let command = parts
        .get(6)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| parts[5].clone());

    Some(RemoteProcessInfo {
        pid,
        user: parts[1].clone(),
        cpu_percent: parse_f64(&parts[2]).map(round_one_decimal).unwrap_or(0.0),
        memory_percent: parse_f64(&parts[3]).map(round_one_decimal).unwrap_or(0.0),
        memory_bytes: parse_u64(&parts[4]).map(|rss_kib| rss_kib.saturating_mul(1024)),
        command,
    })
}

fn parse_disks(sections: &HashMap<String, Vec<String>>) -> Vec<RemoteDiskInfo> {
    if let Some(lines) = sections.get("DF_BYTES") {
        return parse_disk_lines(lines, 1);
    }

    parse_disk_lines(lines_for(sections, "DF_KIB"), 1024)
}

fn parse_disk_lines(lines: &[String], multiplier: u64) -> Vec<RemoteDiskInfo> {
    let mut disks = Vec::new();
    let mut seen_paths = HashSet::new();

    for line in lines {
        if line.to_lowercase().starts_with("filesystem") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            continue;
        }

        let path = parts[5].to_string();
        if !seen_paths.insert(path.clone()) {
            continue;
        }

        disks.push(RemoteDiskInfo {
            filesystem: parts[0].to_string(),
            total_bytes: parse_u64(parts[1]).map(|value| value.saturating_mul(multiplier)),
            used_bytes: parse_u64(parts[2]).map(|value| value.saturating_mul(multiplier)),
            available_bytes: parse_u64(parts[3]).map(|value| value.saturating_mul(multiplier)),
            usage_percent: parse_percent(parts[4]),
            path,
        });
    }

    disks
}

fn parse_uptime_seconds(line: &str) -> Option<u64> {
    line.split_whitespace()
        .next()
        .and_then(parse_f64)
        .map(|value| value.floor() as u64)
}

fn parse_load_average(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .take(3)
        .filter_map(parse_f64)
        .map(round_two_decimals)
        .collect()
}

fn split_whitespace_tail(line: &str, max_parts: usize) -> Vec<String> {
    let mut iterator = line.split_whitespace();
    let mut parts = Vec::new();

    for _ in 0..max_parts.saturating_sub(1) {
        let Some(part) = iterator.next() else {
            return parts;
        };
        parts.push(part.to_string());
    }

    let tail = iterator.collect::<Vec<_>>().join(" ");
    if !tail.is_empty() {
        parts.push(tail);
    }

    parts
}

fn percent_from_parts(used: u64, total: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }

    Some(round_one_decimal(used as f64 * 100.0 / total as f64))
}

fn parse_percent(value: &str) -> Option<f64> {
    parse_f64(value.trim().trim_end_matches('%')).map(round_one_decimal)
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

fn parse_u32(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}

fn parse_f64(value: &str) -> Option<f64> {
    value.trim().replace(',', ".").parse().ok()
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn validate_system_info_request(request: &RemoteSystemInfoRequest) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    Ok(())
}
