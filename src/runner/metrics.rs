use crate::protocol::{Metric, MetricQuality, MetricSource};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, Default)]
pub struct ProcessMetrics {
    pub cpu_percent: Option<Metric<f32>>,
    pub memory_bytes: Option<Metric<u64>>,
}

pub async fn sample_once(pid: u32) -> ProcessMetrics {
    tokio::task::spawn_blocking(move || {
        let mut system = System::new();
        let pid = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        match system.process(pid) {
            Some(process) => ProcessMetrics {
                cpu_percent: Some(Metric::new(
                    process.cpu_usage(),
                    MetricQuality::Exact,
                    MetricSource::Os,
                )),
                memory_bytes: Some(Metric::new(
                    process.memory(),
                    MetricQuality::Exact,
                    MetricSource::Os,
                )),
            },
            None => ProcessMetrics {
                cpu_percent: Some(Metric::new(
                    0.0,
                    MetricQuality::Unavailable,
                    MetricSource::Os,
                )),
                memory_bytes: Some(Metric::new(0, MetricQuality::Unavailable, MetricSource::Os)),
            },
        }
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MetricQuality;

    #[tokio::test]
    async fn sampler_reports_current_process_memory_or_unavailable() {
        let metrics = sample_once(std::process::id()).await;
        let memory = metrics.memory_bytes.unwrap();

        assert!(memory.value > 0 || memory.quality == MetricQuality::Unavailable);
    }

    #[tokio::test]
    async fn missing_process_is_reported_as_unavailable() {
        let metrics = sample_once(u32::MAX).await;

        assert_eq!(
            metrics.memory_bytes.unwrap().quality,
            MetricQuality::Unavailable
        );
        assert_eq!(
            metrics.cpu_percent.unwrap().quality,
            MetricQuality::Unavailable
        );
    }
}
