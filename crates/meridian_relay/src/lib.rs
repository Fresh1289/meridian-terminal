//! Meridian relay bus — atomic Manager↔Builder messaging with approval gates.
//!
//! First-logic slice: in-memory mpsc transport, oneshot-backed approval gate.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RelayType {
    Request,
    Report,
    Blocker,
    Fyi,
    Question,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relay {
    pub id: Uuid,
    pub from: String,
    pub to: String,
    pub kind: RelayType,
    pub body: String,
    pub timestamp: DateTime<Utc>,
    pub needs_approval: bool,
}

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("relay channel closed")]
    ChannelClosed,
    #[error("unknown relay id: {0}")]
    UnknownRelayId(Uuid),
    #[error("relay already approved: {0}")]
    AlreadyApproved(Uuid),
}

struct Envelope {
    relay: Relay,
    ack_rx: Option<oneshot::Receiver<()>>,
}

pub struct RelayBus {
    pending: Mutex<HashMap<Uuid, Option<oneshot::Sender<()>>>>,
}

impl RelayBus {
    // Spec pins `new` to return the split halves rather than Self.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (RelaySender, RelayReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        let bus = Arc::new(RelayBus {
            pending: Mutex::new(HashMap::new()),
        });
        (
            RelaySender {
                tx,
                bus: Arc::clone(&bus),
            },
            RelayReceiver { rx },
        )
    }

    pub fn approve(&self, id: Uuid) -> Result<(), RelayError> {
        let mut pending = self.pending.lock().expect("relay pending map poisoned");
        match pending.get_mut(&id) {
            None => Err(RelayError::UnknownRelayId(id)),
            Some(slot) => {
                let tx = slot.take().ok_or(RelayError::AlreadyApproved(id))?;
                let _ = tx.send(());
                Ok(())
            }
        }
    }
}

pub struct RelaySender {
    tx: mpsc::UnboundedSender<Envelope>,
    bus: Arc<RelayBus>,
}

impl RelaySender {
    pub fn send(&self, relay: Relay) -> Result<(), RelayError> {
        let ack_rx = if relay.needs_approval {
            let (ack_tx, ack_rx) = oneshot::channel();
            self.bus
                .pending
                .lock()
                .expect("relay pending map poisoned")
                .insert(relay.id, Some(ack_tx));
            Some(ack_rx)
        } else {
            None
        };
        self.tx
            .send(Envelope { relay, ack_rx })
            .map_err(|_| RelayError::ChannelClosed)
    }

    pub fn bus(&self) -> &RelayBus {
        &self.bus
    }
}

pub struct RelayReceiver {
    rx: mpsc::UnboundedReceiver<Envelope>,
}

impl RelayReceiver {
    pub async fn recv(&mut self) -> Option<Relay> {
        let envelope = self.rx.recv().await?;
        if let Some(ack_rx) = envelope.ack_rx {
            let _ = ack_rx.await;
        }
        Some(envelope.relay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_relay(needs_approval: bool) -> Relay {
        Relay {
            id: Uuid::new_v4(),
            from: "Manager".into(),
            to: "Builder 1".into(),
            kind: RelayType::Request,
            body: "test body".into(),
            timestamp: Utc::now(),
            needs_approval,
        }
    }

    #[tokio::test]
    async fn send_recv_roundtrip() {
        let (sender, mut receiver) = RelayBus::new();
        let relay = sample_relay(false);
        let id = relay.id;
        sender.send(relay).expect("send ok");
        let received = receiver.recv().await.expect("recv yielded relay");
        assert_eq!(received.id, id);
        assert_eq!(received.from, "Manager");
    }

    #[tokio::test]
    async fn approval_gate_blocks_until_approved() {
        let (sender, mut receiver) = RelayBus::new();
        let relay = sample_relay(true);
        let id = relay.id;
        sender.send(relay).expect("send ok");

        let recv_task = tokio::spawn(async move { receiver.recv().await });
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(!recv_task.is_finished(), "recv resolved before approval");

        sender.bus().approve(id).expect("approve ok");

        let received = tokio::time::timeout(std::time::Duration::from_millis(250), recv_task)
            .await
            .expect("recv resolved after approval")
            .expect("recv task did not panic")
            .expect("recv yielded relay");
        assert_eq!(received.id, id);
    }

    #[tokio::test]
    async fn approve_unknown_id_returns_error() {
        let (sender, _receiver) = RelayBus::new();
        let result = sender.bus().approve(Uuid::new_v4());
        assert!(matches!(result, Err(RelayError::UnknownRelayId(_))));
    }
}
