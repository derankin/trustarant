use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

#[derive(Clone)]
pub struct VoteRateLimiter {
    inner: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl VoteRateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    pub async fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        let queue = guard.entry(key.to_owned()).or_insert_with(VecDeque::new);

        while let Some(front) = queue.front() {
            if now.duration_since(*front) <= self.window {
                break;
            }
            queue.pop_front();
        }

        if queue.len() >= self.max_requests {
            return false;
        }

        queue.push_back(now);
        true
    }
}

