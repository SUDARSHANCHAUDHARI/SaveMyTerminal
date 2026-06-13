mod hybrid;
mod plain;
mod snapshot;

pub use hybrid::HybridRenderer;
pub use plain::PlainRenderer;
pub use snapshot::SnapshotView;

pub trait Renderer: Send {
    fn started(&mut self, agent_id: &str);
    fn finished(&mut self, agent_id: &str, exit_code: i32);
    fn warning(&mut self, message: &str);
}
