use std::sync::Arc;

use tokio::sync::mpsc::{channel, Receiver, Sender};

pub struct Job(pub reqwest::Request);

#[derive(Clone)]
pub struct JobQueue {
    send: Sender<Job>,
}

pub struct JobRunner {
    recv: Receiver<Job>,
    client: reqwest::Client,
}

pub fn create_server(client: reqwest::Client) -> (JobQueue, JobRunner) {
    let (send, recv) = channel(100);
    (JobQueue { send }, JobRunner { recv, client })
}

impl JobRunner {
    pub async fn run(mut self) {
        while let Some(c) = self.recv.recv().await {
            self.process(c).await;
        }
    }

    pub async fn process(&mut self, job: Job) {}
}
