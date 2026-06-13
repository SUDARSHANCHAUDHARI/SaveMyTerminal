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
