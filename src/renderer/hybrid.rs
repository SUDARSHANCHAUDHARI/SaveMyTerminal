use crate::renderer::Renderer;
use std::io::{self, IsTerminal, Write};

pub struct HybridRenderer<W: Write = io::Stderr> {
    writer: W,
    status_enabled: bool,
    ambient_enabled: bool,
}

impl HybridRenderer<io::Stderr> {
    pub fn stderr(status_enabled: bool, ambient_enabled: bool) -> Self {
        let stderr = io::stderr();
        Self {
            ambient_enabled: ambient_enabled && stderr.is_terminal(),
            writer: stderr,
            status_enabled,
        }
    }
}

impl<W: Write> HybridRenderer<W> {
    pub fn new(writer: W, status_enabled: bool, ambient_enabled: bool) -> Self {
        Self {
            writer,
            status_enabled,
            ambient_enabled,
        }
    }

    fn osc(&mut self, value: &str) {
        if self.ambient_enabled {
            let _ = write!(self.writer, "{value}");
        }
    }
}

impl<W: Write + Send> Renderer for HybridRenderer<W> {
    fn started(&mut self, agent_id: &str) {
        if self.status_enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] starting");
        }
        self.osc(&format!(
            "\u{1b}]2;SaveMyTerminal: {agent_id} active\u{7}\u{1b}]12;#8b5cf6\u{7}"
        ));
    }

    fn finished(&mut self, agent_id: &str, exit_code: i32) {
        if self.status_enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] exited {exit_code}");
        }
        self.osc("\u{1b}]2;SaveMyTerminal: idle\u{7}\u{1b}]112\u{7}");
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
        assert!(output.contains("\u{1b}]12;#8b5cf6\u{7}"));
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
}
