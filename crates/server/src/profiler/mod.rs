#[cfg(feature = "profiling")]
mod pprof_profiler;

#[cfg(feature = "profiling")]
pub use pprof_profiler::Profiler;

/// No-op profiler when the `profiling` feature is disabled.
#[cfg(not(feature = "profiling"))]
pub struct Profiler;

#[cfg(not(feature = "profiling"))]
impl Profiler {
    pub fn start() -> Self {
        tracing::debug!("Profiling is disabled. Enable with `--features profiling`.");
        Self
    }

    pub fn stop_and_report(self) {
        tracing::debug!("Profiling is disabled; nothing to report.");
    }
}
