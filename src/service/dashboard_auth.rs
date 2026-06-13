use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct DashboardAuth {
    inner: Arc<Mutex<AuthState>>,
    launch_ttl: Duration,
}

#[derive(Default)]
struct AuthState {
    launch_tokens: HashMap<String, Instant>,
    sessions: HashSet<String>,
}

impl DashboardAuth {
    pub fn new(launch_ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AuthState::default())),
            launch_ttl,
        }
    }

    pub fn create_launch_token(&self) -> String {
        let token = random_token();
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        remove_expired(&mut state);
        state
            .launch_tokens
            .insert(token.clone(), Instant::now() + self.launch_ttl);
        token
    }

    pub fn consume_launch_token(&self, token: &str) -> Option<String> {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        remove_expired(&mut state);
        state.launch_tokens.remove(token)?;
        let session = random_token();
        state.sessions.insert(session.clone());
        Some(session)
    }

    pub fn validates_session(&self, session: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sessions
            .contains(session)
    }
}

fn remove_expired(state: &mut AuthState) {
    let now = Instant::now();
    state
        .launch_tokens
        .retain(|_, expires_at| *expires_at > now);
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
