use crate::renderer::Renderer;
use std::io::{self, Write};

pub struct PlainRenderer<W: Write = io::Stderr> {
    writer: W,
    enabled: bool,
}

impl PlainRenderer<io::Stderr> {
    pub fn stderr(enabled: bool) -> Self {
        Self {
            writer: io::stderr(),
            enabled,
        }
    }
}

impl<W: Write + Send> Renderer for PlainRenderer<W> {
    fn started(&mut self, agent_id: &str) {
        if self.enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] starting");
        }
    }

    fn finished(&mut self, agent_id: &str, exit_code: i32) {
        if self.enabled {
            let _ = writeln!(self.writer, "smt [{agent_id}] exited {exit_code}");
        }
    }

    fn warning(&mut self, message: &str) {
        if self.enabled {
            let _ = writeln!(self.writer, "smt warning: {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_renderer_writes_nothing() {
        let mut output = Vec::new();
        {
            let mut renderer = PlainRenderer {
                writer: &mut output,
                enabled: false,
            };
            renderer.started("codex");
            renderer.warning("offline");
            renderer.finished("codex", 0);
        }

        assert!(output.is_empty());
    }

    #[test]
    fn renderer_outputs_only_agent_state_metadata() {
        let mut output = Vec::new();
        {
            let mut renderer = PlainRenderer {
                writer: &mut output,
                enabled: true,
            };
            renderer.started("codex");
            renderer.finished("codex", 0);
        }

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "smt [codex] starting\nsmt [codex] exited 0\n"
        );
    }
}
