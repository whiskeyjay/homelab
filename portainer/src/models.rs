use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub(crate) struct Endpoint {
    #[serde(rename = "Id")]
    pub(crate) id: i32,

    #[serde(rename = "Name")]
    pub(crate) name: String,
}

#[derive(Deserialize)]
pub(crate) struct Container {
    #[serde(rename = "Id")]
    pub(crate) id: String,
    #[serde(rename = "Names")]
    pub(crate) names: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct CpuStats {
    pub(crate) cpu_usage: CpuUsage,
    pub(crate) system_cpu_usage: Option<u64>,
    pub(crate) online_cpus: u64,
    #[serde(rename = "throttling_data")]
    pub(crate) throttling_data: ThrottlingData,
}

#[derive(Deserialize)]
pub(crate) struct CpuUsage {
    pub(crate) total_usage: u64,
    pub(crate) usage_in_kernelmode: u64,
    pub(crate) usage_in_usermode: u64,
    #[allow(dead_code)]
    pub(crate) percpu_usage: Option<Vec<u64>>,
}

#[derive(Deserialize)]
pub(crate) struct ThrottlingData {
    pub(crate) periods: u64,
    pub(crate) throttled_periods: u64,
    pub(crate) throttled_time: u64,
}

#[derive(Deserialize)]
pub(crate) struct MemoryStats {
    pub(crate) usage: u64,
    pub(crate) limit: u64,
    pub(crate) stats: MemoryStatsDetails,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct MemoryStatsDetails {
    #[serde(default)]
    pub(crate) active_anon: u64,
    #[serde(default)]
    pub(crate) active_file: u64,
    #[serde(default)]
    pub(crate) anon: u64,
    #[serde(default)]
    pub(crate) anon_thp: u64,
    #[serde(default)]
    pub(crate) file: u64,
    #[serde(default)]
    pub(crate) file_dirty: u64,
    #[serde(default)]
    pub(crate) file_mapped: u64,
    #[serde(default)]
    pub(crate) file_writeback: u64,
    #[serde(default)]
    pub(crate) inactive_anon: u64,
    #[serde(default)]
    pub(crate) inactive_file: u64,
    #[serde(default)]
    pub(crate) kernel_stack: u64,
    #[serde(default)]
    pub(crate) pgactivate: u64,
    #[serde(default)]
    pub(crate) pgdeactivate: u64,
    #[serde(default)]
    pub(crate) pgfault: u64,
    #[serde(default)]
    pub(crate) pglazyfree: u64,
    #[serde(default)]
    pub(crate) pglazyfreed: u64,
    #[serde(default)]
    pub(crate) pgmajfault: u64,
    #[serde(default)]
    pub(crate) pgrefill: u64,
    #[serde(default)]
    pub(crate) pgscan: u64,
    #[serde(default)]
    pub(crate) pgsteal: u64,
    #[serde(default)]
    pub(crate) shmem: u64,
    #[serde(default)]
    pub(crate) slab: u64,
    #[serde(default)]
    pub(crate) slab_reclaimable: u64,
    #[serde(default)]
    pub(crate) slab_unreclaimable: u64,
    #[serde(default)]
    pub(crate) sock: u64,
    #[serde(default)]
    pub(crate) thp_collapse_alloc: u64,
    #[serde(default)]
    pub(crate) thp_fault_alloc: u64,
    #[serde(default)]
    pub(crate) unevictable: u64,
    #[serde(default)]
    pub(crate) workingset_activate: u64,
    #[serde(default)]
    pub(crate) workingset_nodereclaim: u64,
    #[serde(default)]
    pub(crate) workingset_refault: u64,
}

#[derive(Deserialize)]
pub(crate) struct NetworkStats {
    pub(crate) rx_bytes: u64,
    pub(crate) rx_packets: u64,
    pub(crate) rx_errors: u64,
    pub(crate) rx_dropped: u64,
    pub(crate) tx_bytes: u64,
    pub(crate) tx_packets: u64,
    pub(crate) tx_errors: u64,
    pub(crate) tx_dropped: u64,
}

#[derive(Deserialize)]
pub(crate) struct BlkioStats {
    #[serde(default)]
    pub(crate) io_service_bytes_recursive: Vec<BlkioStat>,
}

#[derive(Deserialize)]
pub(crate) struct BlkioStat {
    pub(crate) major: u64,
    pub(crate) minor: u64,
    pub(crate) op: String,
    pub(crate) value: u64,
}

#[derive(Deserialize)]
pub(crate) struct Stats {
    #[allow(dead_code)]
    pub(crate) read: String,
    pub(crate) cpu_stats: CpuStats,
    pub(crate) precpu_stats: CpuStats,
    pub(crate) memory_stats: MemoryStats,
    pub(crate) networks: Option<HashMap<String, NetworkStats>>,
    pub(crate) blkio_stats: BlkioStats,
    pub(crate) pids_stats: HashMap<String, u64>,
}
