use std::hint::black_box;
use std::time::Instant;

use diagnostic_parser::DiagnosticReport;

#[cfg(target_os = "macos")]
fn peak_rss_bytes() -> Option<u64> {
    use std::mem::zeroed;
    unsafe {
        let mut usage: libc::rusage = zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            Some(usage.ru_maxrss as u64)
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn peak_rss_bytes() -> Option<u64> {
    use std::mem::zeroed;
    unsafe {
        let mut usage: libc::rusage = zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            Some(usage.ru_maxrss as u64 * 1024)
        } else {
            None
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

fn rss_mb() -> String {
    match peak_rss_bytes() {
        Some(bytes) => format!("{:.2} MB", bytes as f64 / 1_048_576.0),
        None => "unavailable".to_string(),
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: cargo run --example bench -- <path-to-.1pdiagnostics>");

    println!("Peak RSS at startup: {}", rss_mb());
    println!();

    // ── Phase 1: Read file ───────────────────────────────────────────
    let start = Instant::now();
    let data = std::fs::read_to_string(&path).expect("failed to read file");
    let read_elapsed = start.elapsed();
    let file_size = data.len();

    println!(
        "Phase 1 — Read file: {:.2} MB in {:.2} ms",
        file_size as f64 / 1_048_576.0,
        read_elapsed.as_secs_f64() * 1000.0,
    );
    println!("  Peak RSS after read: {}", rss_mb());
    println!();

    // ── Phase 2: JSON deserialization ────────────────────────────────
    let start = Instant::now();
    let report = black_box(&data)
        .parse::<DiagnosticReport>()
        .expect("failed to parse JSON");
    let json_elapsed = start.elapsed();

    let total_log_bytes: usize = report.logs.iter().map(|lf| lf.content.len()).sum();
    let total_log_lines = report.total_log_lines();
    let metadata_bytes = file_size.saturating_sub(total_log_bytes);

    println!(
        "Phase 2 — JSON deserialization: {:.2} ms",
        json_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "  Log content: {:.2} MB across {} files ({total_log_lines} lines)",
        total_log_bytes as f64 / 1_048_576.0,
        report.logs.len(),
    );
    println!(
        "  Metadata (non-log): {:.2} KB",
        metadata_bytes as f64 / 1024.0,
    );
    println!("  Peak RSS after JSON deser: {}", rss_mb());

    // Drop the raw JSON string — the report now owns the log content.
    drop(data);
    println!("  Peak RSS after dropping raw JSON: {}", rss_mb());
    println!();

    // ── Phase 3a: Owned log parsing ──────────────────────────────────
    let start = Instant::now();
    let owned_entries = report.parse_log_entries();
    let owned_elapsed = start.elapsed();

    let owned_count = owned_entries.len();
    let owned_continuations: usize = owned_entries.iter().map(|e| e.continuation.len()).sum();
    let owned_heap_bytes: usize = owned_entries
        .iter()
        .map(|e| {
            e.log_file_title.capacity()
                + e.thread.capacity()
                + e.source.component.capacity()
                + e.source.detail.as_ref().map_or(0, |d| d.capacity())
                + e.message.capacity()
                + e.continuation.iter().map(|c| c.capacity()).sum::<usize>()
                + e.continuation.capacity() * std::mem::size_of::<String>()
        })
        .sum();
    let owned_vec_bytes = owned_entries.capacity() * std::mem::size_of_val(&owned_entries[0]);

    println!(
        "Phase 3a — Owned parse: {owned_count} entries in {:.2} ms",
        owned_elapsed.as_secs_f64() * 1000.0,
    );
    println!("  Continuation lines: {owned_continuations}");
    println!(
        "  sizeof(LogEntry): {} bytes",
        std::mem::size_of_val(&owned_entries[0])
    );
    println!(
        "  String heap data:  {:.2} MB",
        owned_heap_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Vec<LogEntry>:     {:.2} MB",
        owned_vec_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Total owned alloc: {:.2} MB",
        (owned_heap_bytes + owned_vec_bytes) as f64 / 1_048_576.0
    );
    println!("  Peak RSS after owned parse: {}", rss_mb());

    drop(owned_entries);
    println!("  Peak RSS after dropping owned: {}", rss_mb());
    println!();

    // ── Phase 3b: Zero-copy log parsing ──────────────────────────────
    let start = Instant::now();
    let (ref_entries, interner) = report.parse_log_entries_ref();
    let ref_elapsed = start.elapsed();

    let ref_count = ref_entries.len();
    let ref_continuations: usize = ref_entries.iter().map(|e| e.continuation.len()).sum();

    // LogEntryRef heap cost: just the Vec itself + continuation Vec per entry
    // (no String heap for message/source/etc. — those are &str slices).
    // Arc<str> costs are shared via the interner.
    let ref_entry_size = std::mem::size_of_val(&ref_entries[0]);
    let ref_vec_bytes = ref_entries.capacity() * ref_entry_size;
    let ref_continuation_vec_bytes: usize = ref_entries
        .iter()
        .map(|e| e.continuation.capacity() * std::mem::size_of::<&str>())
        .sum();

    println!(
        "Phase 3b — Zero-copy parse: {ref_count} entries in {:.2} ms",
        ref_elapsed.as_secs_f64() * 1000.0,
    );
    println!("  Continuation lines: {ref_continuations}");
    println!("  Interned strings: {}", interner.len());
    println!("  sizeof(LogEntryRef): {} bytes", ref_entry_size);
    println!(
        "  Vec<LogEntryRef>:       {:.2} MB",
        ref_vec_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Continuation Vec heap:  {:.2} KB",
        ref_continuation_vec_bytes as f64 / 1024.0
    );
    println!(
        "  Total zero-copy alloc:  {:.2} MB",
        (ref_vec_bytes + ref_continuation_vec_bytes) as f64 / 1_048_576.0
    );
    println!("  Peak RSS after ref parse: {}", rss_mb());
    println!();

    // ── Comparison ───────────────────────────────────────────────────
    let owned_total = owned_heap_bytes + owned_vec_bytes;
    let ref_total = ref_vec_bytes + ref_continuation_vec_bytes;
    let savings_bytes = owned_total.saturating_sub(ref_total);
    let savings_pct = if owned_total > 0 {
        savings_bytes as f64 / owned_total as f64 * 100.0
    } else {
        0.0
    };
    let speedup = if ref_elapsed.as_nanos() > 0 {
        owned_elapsed.as_secs_f64() / ref_elapsed.as_secs_f64()
    } else {
        0.0
    };

    println!("═══ Owned vs. Zero-Copy Comparison ═══");
    println!(
        "  Time:     {:.2} ms (owned) vs {:.2} ms (zero-copy) — {speedup:.2}× faster",
        owned_elapsed.as_secs_f64() * 1000.0,
        ref_elapsed.as_secs_f64() * 1000.0,
    );
    println!(
        "  Alloc:    {:.2} MB (owned) vs {:.2} MB (zero-copy) — {:.2} MB saved ({savings_pct:.0}%)",
        owned_total as f64 / 1_048_576.0,
        ref_total as f64 / 1_048_576.0,
        savings_bytes as f64 / 1_048_576.0,
    );
    println!();

    // ── Memory waterfall at peak (zero-copy path) ────────────────────
    let model_log_content = total_log_bytes;
    let peak_ref = model_log_content + metadata_bytes + ref_total;

    println!("═══ Memory Waterfall (zero-copy path at peak) ═══");
    println!(
        "  DiagnosticReport.logs content: {:.2} MB",
        model_log_content as f64 / 1_048_576.0
    );
    println!(
        "  DiagnosticReport metadata:     {:.2} KB",
        metadata_bytes as f64 / 1024.0
    );
    println!(
        "  Vec<LogEntryRef> + continuations: {:.2} MB",
        ref_total as f64 / 1_048_576.0
    );
    println!("  ─────────────────────────────────────────");
    println!(
        "  Estimated peak heap:           {:.2} MB",
        peak_ref as f64 / 1_048_576.0
    );
    println!();

    // Keep entries alive for RSS measurement, then drop.
    drop(black_box(ref_entries));
    drop(black_box(interner));

    // ── Throughput benchmark ─────────────────────────────────────────
    let iterations = 20;

    // Warm up.
    let _ = black_box(report.parse_log_entries());
    let (r, _) = report.parse_log_entries_ref();
    let _ = black_box(r);

    // Owned throughput.
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = black_box(report.parse_log_entries());
    }
    let owned_bench = start.elapsed();

    // Zero-copy throughput.
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = black_box(report.parse_log_entries_ref());
    }
    let ref_bench = start.elapsed();

    let owned_ms = owned_bench.as_secs_f64() / iterations as f64 * 1000.0;
    let ref_ms = ref_bench.as_secs_f64() / iterations as f64 * 1000.0;
    let owned_entries_sec = owned_count as f64 / (owned_bench.as_secs_f64() / iterations as f64);
    let ref_entries_sec = ref_count as f64 / (ref_bench.as_secs_f64() / iterations as f64);
    let owned_mb_sec =
        (total_log_bytes as f64 / 1_048_576.0) / (owned_bench.as_secs_f64() / iterations as f64);
    let ref_mb_sec =
        (total_log_bytes as f64 / 1_048_576.0) / (ref_bench.as_secs_f64() / iterations as f64);
    let bench_speedup = if ref_ms > 0.0 { owned_ms / ref_ms } else { 0.0 };

    println!("═══ Throughput ({iterations} iterations) ═══");
    println!("                    Owned          Zero-Copy");
    println!("  ms/iter:         {owned_ms:7.2}          {ref_ms:7.2}");
    println!("  entries/sec:  {owned_entries_sec:10.0}       {ref_entries_sec:10.0}");
    println!("  MB/sec:        {owned_mb_sec:8.1}         {ref_mb_sec:8.1}");
    println!("  Speedup:       {bench_speedup:.2}×");

    // Keep report alive so RSS measurements above are meaningful.
    drop(black_box(report));
}
