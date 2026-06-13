mod plain;

pub use plain::PlainRenderer;

pub trait Renderer: Send {
    fn started(&mut self, agent_id: &str);
    fn finished(&mut self, agent_id: &str, exit_code: i32);
    fn warning(&mut self, message: &str);
}
