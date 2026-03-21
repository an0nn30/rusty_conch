//! MobileRemoteCallbacks — bridges RemoteCallbacks to Tauri events.
//!
//! When the SSH handler needs user interaction (host key confirmation,
//! password entry), this emits a Tauri event and waits on a oneshot
//! channel that the frontend resolves via auth_respond commands.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::Emitter;

use conch_remote::callbacks::RemoteCallbacks;
use crate::state::PendingPrompts;

#[derive(Clone, Serialize)]
pub struct HostKeyPromptEvent {
    pub prompt_id: String,
    pub message: String,
    pub detail: String,
}

#[derive(Clone, Serialize)]
pub struct PasswordPromptEvent {
    pub prompt_id: String,
    pub message: String,
}

/// Bridges `RemoteCallbacks` to Tauri events + oneshot channels.
pub struct MobileRemoteCallbacks {
    pub app: tauri::AppHandle,
    pub pending_prompts: Arc<Mutex<PendingPrompts>>,
}

#[async_trait::async_trait]
impl RemoteCallbacks for MobileRemoteCallbacks {
    async fn verify_host_key(&self, message: &str, fingerprint: &str) -> bool {
        let prompt_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_prompts
            .lock()
            .host_key
            .insert(prompt_id.clone(), tx);
        let _ = self.app.emit(
            "ssh-host-key-prompt",
            HostKeyPromptEvent {
                prompt_id,
                message: message.to_string(),
                detail: fingerprint.to_string(),
            },
        );
        rx.await.unwrap_or(false)
    }

    async fn prompt_password(&self, message: &str) -> Option<String> {
        let prompt_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_prompts
            .lock()
            .password
            .insert(prompt_id.clone(), tx);
        let _ = self.app.emit(
            "ssh-password-prompt",
            PasswordPromptEvent {
                prompt_id,
                message: message.to_string(),
            },
        );
        rx.await.unwrap_or(None)
    }

    fn on_transfer_progress(&self, _transfer_id: &str, _bytes: u64, _total: Option<u64>) {
        // Transfer progress handled separately — not needed for SSH wiring.
    }
}
