use std::{fs::File, io::Write};

use pprof::{ProfilerGuard, protos::Message};
use tracing::{error, info};

const PROFILING_FREQUENCY: i32 = 99;
const FLAMEGRAPH_OUTPUT: &str = "flamegraph.svg";
const PPROF_OUTPUT: &str = "profile.pb";

/// CPU profiler backed by pprof-rs.
///
/// Created via [`Profiler::start`], which begins sampling immediately.
/// Call [`Profiler::stop_and_report`] to finalize and write output files.
pub struct Profiler {
    guard: ProfilerGuard<'static>,
}

impl Profiler {
    /// Begin CPU profiling at the default frequency.
    pub fn start() -> Self {
        info!(frequency = PROFILING_FREQUENCY, "Starting pprof CPU profiler");

        let guard = pprof::ProfilerGuardBuilder::default()
            .frequency(PROFILING_FREQUENCY)
            .blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build()
            .expect("Failed to start pprof profiler");

        Self { guard }
    }

    /// Stop profiling and write reports (flamegraph SVG + pprof protobuf).
    pub fn stop_and_report(self) {
        info!("Stopping profiler and generating reports…");

        // Build a report with a frames post-processor that enriches thread names.
        // The closure is invoked for every resolved `Frames` before aggregation.
        match self.guard.report().frames_post_processor(frames_post_processor).build() {
            Ok(report) => {
                Self::write_flamegraph(&report);
                Self::write_pprof_proto(&report);
            }
            Err(e) => {
                error!("Failed to build profiler report: {e}");
            }
        }
    }

    /// Generate an interactive flamegraph SVG.
    fn write_flamegraph(report: &pprof::Report) {
        match File::create(FLAMEGRAPH_OUTPUT) {
            Ok(mut f) => {
                if let Err(e) = report.flamegraph(&mut f) {
                    error!(path = FLAMEGRAPH_OUTPUT, "Failed to write flamegraph: {e}");
                } else {
                    info!(path = FLAMEGRAPH_OUTPUT, "Flamegraph written successfully");
                }
            }
            Err(e) => {
                error!(path = FLAMEGRAPH_OUTPUT, "Failed to create flamegraph file: {e}");
            }
        }
    }

    /// Write a pprof-compatible protobuf profile (readable by `go tool pprof`).
    fn write_pprof_proto(report: &pprof::Report) {
        let profile = match report.pprof() {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to generate pprof protobuf: {e}");
                return;
            }
        };

        let mut buf = Vec::new();
        if let Err(e) = profile.write_to_vec(&mut buf) {
            error!("Failed to serialize pprof protobuf: {e}");
            return;
        }

        match File::create(PPROF_OUTPUT) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(&buf) {
                    error!(path = PPROF_OUTPUT, "Failed to write pprof protobuf: {e}");
                } else {
                    info!(
                        path = PPROF_OUTPUT,
                        "pprof protobuf written (use `go tool pprof {PPROF_OUTPUT}` to analyze)"
                    );
                }
            }
            Err(e) => {
                error!(path = PPROF_OUTPUT, "Failed to create pprof output file: {e}");
            }
        }
    }
}

/// Frame post-processor that normalises thread names.
///
/// If the thread name is empty (unnamed threads), it is replaced with
/// `"thread-<id>"` so that flamegraphs and pprof outputs always show
/// a human-readable identifier.
fn frames_post_processor(frames: &mut pprof::Frames) {
    if frames.thread_name.is_empty() {
        frames.thread_name = format!("thread-{}", frames.thread_id);
    }
}
