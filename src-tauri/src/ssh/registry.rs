use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tokio::sync::{mpsc, oneshot};

use crate::models::{
    RemoteDirectoryListing, RemoteDownloadRequest, RemoteSystemSnapshot, RemoteTransferEntry,
    RemoteUploadRequest,
};
use crate::util::error_message;

#[derive(Clone, Default)]
pub struct TerminalRegistry {
    sessions: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<TerminalControl>>>>,
    pub(crate) transfer_cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

pub(crate) enum TerminalControl {
    Input(Vec<u8>),
    Resize { columns: u32, rows: u32 },
    ListDirectory {
        path: String,
        responder: oneshot::Sender<Result<RemoteDirectoryListing, String>>,
    },
    GetSystemInfo {
        responder: oneshot::Sender<Result<RemoteSystemSnapshot, String>>,
    },
    CreateDirectory {
        path: String,
        responder: oneshot::Sender<Result<(), String>>,
    },
    RenamePath {
        from_path: String,
        to_path: String,
        responder: oneshot::Sender<Result<(), String>>,
    },
    DeletePaths {
        entries: Vec<RemoteTransferEntry>,
        recursive: bool,
        responder: oneshot::Sender<Result<(), String>>,
    },
    StartUpload {
        request: RemoteUploadRequest,
        cancellation: Arc<AtomicBool>,
    },
    StartDownload {
        request: RemoteDownloadRequest,
        cancellation: Arc<AtomicBool>,
    },
    Close,
}

impl TerminalRegistry {
    pub(crate) fn replace(
        &self,
        session_id: String,
        sender: mpsc::UnboundedSender<TerminalControl>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(error_message)?;
        if let Some(existing) = sessions.insert(session_id, sender) {
            let _ = existing.send(TerminalControl::Close);
        }

        Ok(())
    }

    pub(crate) fn send(&self, session_id: &str, control: TerminalControl) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(error_message)?;
        let sender = sessions
            .get(session_id)
            .ok_or_else(|| "Terminal session is not running".to_string())?;

        sender
            .send(control)
            .map_err(|_| "Terminal session is not accepting input".to_string())
    }

    pub(crate) fn close(&self, session_id: &str) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .remove(session_id);

        if let Some(sender) = sender {
            let _ = sender.send(TerminalControl::Close);
        }

        Ok(())
    }

    pub(crate) async fn list_directory(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<RemoteDirectoryListing, String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ListDirectory { path, responder })
            .map_err(|_| "Terminal session is not accepting SFTP requests".to_string())?;

        receiver
            .await
            .map_err(|_| "SFTP request was canceled".to_string())?
    }

    pub(crate) async fn get_system_info(&self, session_id: &str) -> Result<RemoteSystemSnapshot, String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::GetSystemInfo { responder })
            .map_err(|_| "Terminal session is not accepting system info requests".to_string())?;

        receiver
            .await
            .map_err(|_| "System info request was canceled".to_string())?
    }

    pub(crate) async fn create_directory(&self, session_id: &str, path: String) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::CreateDirectory { path, responder })
            .map_err(|_| "Terminal session is not accepting file requests".to_string())?;

        receiver
            .await
            .map_err(|_| "Create directory request was canceled".to_string())?
    }

    pub(crate) async fn rename_path(
        &self,
        session_id: &str,
        from_path: String,
        to_path: String,
    ) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::RenamePath {
                from_path,
                to_path,
                responder,
            })
            .map_err(|_| "Terminal session is not accepting file requests".to_string())?;

        receiver
            .await
            .map_err(|_| "Rename request was canceled".to_string())?
    }

    pub(crate) async fn delete_paths(
        &self,
        session_id: &str,
        entries: Vec<RemoteTransferEntry>,
        recursive: bool,
    ) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::DeletePaths {
                entries,
                recursive,
                responder,
            })
            .map_err(|_| "Terminal session is not accepting file requests".to_string())?;

        receiver
            .await
            .map_err(|_| "Delete request was canceled".to_string())?
    }

    pub(crate) fn start_upload(&self, request: RemoteUploadRequest) -> Result<(), String> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(&request.session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(error_message)?
            .insert(task_id.clone(), Arc::clone(&cancellation));

        let send_result = sender
            .send(TerminalControl::StartUpload {
                request,
                cancellation,
            })
            .map_err(|_| "Terminal session is not accepting upload requests".to_string());

        if send_result.is_err() {
            self.transfer_cancellations
                .lock()
                .map_err(error_message)?
                .remove(&task_id);
        }

        send_result
    }

    pub(crate) fn start_download(&self, request: RemoteDownloadRequest) -> Result<(), String> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(&request.session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(error_message)?
            .insert(task_id.clone(), Arc::clone(&cancellation));

        let send_result = sender
            .send(TerminalControl::StartDownload {
                request,
                cancellation,
            })
            .map_err(|_| "Terminal session is not accepting download requests".to_string());

        if send_result.is_err() {
            self.transfer_cancellations
                .lock()
                .map_err(error_message)?
                .remove(&task_id);
        }

        send_result
    }

    pub(crate) fn cancel_transfer(&self, task_id: &str) -> Result<(), String> {
        let cancellations = self.transfer_cancellations.lock().map_err(error_message)?;
        if let Some(cancellation) = cancellations.get(task_id) {
            cancellation.store(true, Ordering::SeqCst);
        }

        Ok(())
    }
}
