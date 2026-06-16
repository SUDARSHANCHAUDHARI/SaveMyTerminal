use crate::{
    protocol::SessionState,
    renderer::{Renderer, SnapshotView},
};
use std::io::{self, IsTerminal, Write};

pub struct HybridRenderer<W: Write = io::Stderr> {
    writer: W,
    status_enabled: bool,
    ambient_enabled: bool,
    ambient_intensity: u8,
    last_signal: Option<String>,
}

impl HybridRenderer<io::Stderr> {
    pub fn stderr(status_enabled: bool, ambient_enabled: bool, ambient_intensity: u8) -> Self {
        let stderr = io::stderr();
        Self {
            ambient_enabled: ambient_enabled && stderr.is_terminal(),
            writer: stderr,
            status_enabled,
            ambient_intensity: ambient_intensity.min(100),
            last_signal: None,
        }
    }
}

impl<W: Write> HybridRenderer<W> {
    pub fn new(writer: W, status_enabled: bool, ambient_enabled: bool) -> Self {
        Self {
            writer,
            status_enabled,
            ambient_enabled,
            ambient_intensity: 60,
            last_signal: None,
        }
    }

    fn osc(&mut self, value: &str) {
        if self.ambient_enabled {
            let _ = write!(self.writer, "{value}");
        }
    }

    pub fn snapshot(&mut self, view: &SnapshotView) {
        let Some(state) = view.state else {
            return;
        };
        let signal = encode_signal(
            state,
            view.intensity,
            view.context_pressure.as_ref().map(|m| m.value),
        );
        if self.last_signal.as_deref() == Some(&signal) {
            return;
        }
        self.osc(&format!(
            "\u{1b}]2;{}\u{7}\u{1b}]12;{signal}\u{7}",
            view.label
        ));
        self.last_signal = Some(signal);
    }
}

fn encode_signal(state: SessionState, intensity: u8, context_pressure: Option<f32>) -> String {
    let state_code = 0xa0_u8
        + match state {
            SessionState::Starting => 0,
            SessionState::Thinking => 1,
            SessionState::ToolRunning => 2,
            SessionState::Waiting => 3,
            SessionState::Completed => 4,
            SessionState::Failed => 5,
            SessionState::Interrupted => 6,
        };
    let intensity = ((f32::from(intensity.min(100)) / 100.0) * 255.0).round() as u8;
    let context = context_pressure
        .filter(|value| value.is_finite())
        .map(|value| ((value.clamp(0.0, 100.0) / 100.0) * 254.0).round() as u8)
        .unwrap_or(255);
    format!("#{state_code:02x}{intensity:02x}{context:02x}")
}

impl<W: Write + Send> Renderer for HybridRenderer<W> {
    fn started(&mut self, agent_id: &str) {
        if self.status_enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] starting");
        }
        let signal = encode_signal(SessionState::Starting, self.ambient_intensity, None);
        self.osc(&format!(
            "\u{1b}]2;SaveMyTerminal: {agent_id} active\u{7}\u{1b}]12;{signal}\u{7}"
        ));
        self.last_signal = Some(signal);
    }

    fn snapshot(&mut self, view: &SnapshotView) {
        HybridRenderer::snapshot(self, view);
    }

    fn finished(&mut self, agent_id: &str, exit_code: i32) {
        if self.status_enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] exited {exit_code}");
        }
        self.osc("\u{1b}]2;SaveMyTerminal: idle\u{7}\u{1b}]112\u{7}");
        self.last_signal = None;
    }

    fn warning(&mut self, message: &str) {
        if self.status_enabled {
            let _ = writeln!(self.writer, "smt warning: {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_renderer_sets_and_cleans_up_terminal_state() {
        let mut output = Vec::new();
        {
            let mut renderer = HybridRenderer::new(&mut output, false, true);
            renderer.started("codex");
            renderer.finished("codex", 0);
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("SaveMyTerminal: codex active"));
        assert!(output.contains(&encode_signal(SessionState::Starting, 60, None)));
        assert!(output.contains("\u{1b}]112\u{7}"));
        assert!(!output.contains("smt [codex]"));
    }

    #[test]
    fn disabled_ambient_preserves_plain_status_only() {
        let mut output = Vec::new();
        {
            let mut renderer = HybridRenderer::new(&mut output, true, false);
            renderer.started("claude");
            renderer.finished("claude", 2);
        }
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "smt [claude] starting\nsmt [claude] exited 2\n"
        );
    }

    #[test]
    fn ambient_renderer_emits_distinct_state_and_context_signals() {
        use crate::protocol::{Metric, MetricQuality, MetricSource, SessionState};
        use crate::renderer::SnapshotView;

        let mut output = Vec::new();
        let mut renderer = HybridRenderer::new(&mut output, false, true);
        let view = SnapshotView {
            active_count: 1,
            agent_id: Some("codex".to_owned()),
            state: Some(SessionState::Thinking),
            label: "smt codex thinking".to_owned(),
            color: "#8b5cf6".to_owned(),
            intensity: 60,
            context_pressure: Some(Metric::new(75.0, MetricQuality::Exact, MetricSource::Agent)),
        };

        let thinking_signal = encode_signal(
            SessionState::Thinking,
            60,
            view.context_pressure.as_ref().map(|metric| metric.value),
        );
        renderer.snapshot(&view);
        renderer.snapshot(&SnapshotView {
            state: Some(SessionState::Waiting),
            label: "smt codex waiting".to_owned(),
            ..view
        });
        drop(renderer);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(&thinking_signal));
        assert!(output.contains(&encode_signal(SessionState::Waiting, 60, Some(75.0))));
    }
}
