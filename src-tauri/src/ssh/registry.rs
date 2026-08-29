use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tokio::sync::{mpsc, oneshot};

use crate::error::{lock_poisoned, not_accepting, request_canceled, session_not_running, AppError};
use crate::models::{
    RemoteDownloadRequest, RemoteFileContent, RemoteFileEntry, RemoteSystemSnapshot,
    RemoteTransferEntry, RemoteUploadRequest,
};

#[derive(Clone, Default)]
pub struct TerminalRegistry {
    sessions: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<TerminalControl>>>>,
    pub(crate) transfer_cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    directory_streams: Arc<Mutex<HashMap<String, (String, Arc<AtomicBool>)>>>,
}

pub(crate) enum TerminalControl {
    Input(Vec<u8>),
    Resize {
        columns: u32,
        rows: u32,
    },
    StreamDirectory {
        request_id: String,
        path: String,
        cancellation: Arc<AtomicBool>,
    },
    GetSystemInfo {
        responder: oneshot::Sender<Result<RemoteSystemSnapshot, AppError>>,
    },
    CreateDirectory {
        path: String,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    CreateFile {
        path: String,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    ListDirectory {
        path: String,
        responder: oneshot::Sender<Result<Vec<RemoteFileEntry>, AppError>>,
    },
    ReadFileText {
        path: String,
        responder: oneshot::Sender<Result<RemoteFileContent, AppError>>,
    },
    WriteFileText {
        path: String,
        content: String,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    RenamePath {
        from_path: String,
        to_path: String,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    DeletePaths {
        entries: Vec<RemoteTransferEntry>,
        recursive: bool,
        responder: oneshot::Sender<Result<(), AppError>>,
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
    ) -> Result<(), AppError> {
        self.cancel_directory_stream(&session_id);

        let mut sessions = self.sessions.lock().map_err(|_| lock_poisoned())?;
        if let Some(existing) = sessions.insert(session_id, sender) {
            let _ = existing.send(TerminalControl::Close);
        }

        Ok(())
    }

    pub(crate) fn send(&self, session_id: &str, control: TerminalControl) -> Result<(), AppError> {
        let sessions = self.sessions.lock().map_err(|_| lock_poisoned())?;
        let sender = sessions.get(session_id).ok_or_else(session_not_running)?;

        sender.send(control).map_err(|_| not_accepting("input"))
    }

    pub(crate) fn close(&self, session_id: &str) -> Result<(), AppError> {
        self.cancel_directory_stream(session_id);

        let sender = self
            .sessions
            .lock()
            .map_err(|_| lock_poisoned())?
            .remove(session_id);

        if let Some(sender) = sender {
            let _ = sender.send(TerminalControl::Close);
        }

        Ok(())
    }

    /// 取一份会话控制通道的克隆；会话不存在时报 `session.notRunning`。
    fn sender_for(
        &self,
        session_id: &str,
    ) -> Result<mpsc::UnboundedSender<TerminalControl>, AppError> {
        self.sessions
            .lock()
            .map_err(|_| lock_poisoned())?
            .get(session_id)
            .cloned()
            .ok_or_else(session_not_running)
    }

    pub(crate) fn stream_directory(
        &self,
        session_id: &str,
        request_id: String,
        path: String,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let cancellation = Arc::new(AtomicBool::new(false));

        if let Some((_, previous)) = self
            .directory_streams
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(
                session_id.to_string(),
                (request_id.clone(), Arc::clone(&cancellation)),
            )
        {
            previous.store(true, Ordering::SeqCst);
        }

        let send_result = sender
            .send(TerminalControl::StreamDirectory {
                request_id: request_id.clone(),
                path,
                cancellation,
            })
            .map_err(|_| not_accepting("sftp"));

        if send_result.is_err() {
            self.finish_directory_stream(session_id, &request_id);
        }

        send_result
    }

    pub(crate) fn cancel_directory_stream(&self, session_id: &str) {
        if let Ok(mut streams) = self.directory_streams.lock() {
            if let Some((_, cancellation)) = streams.remove(session_id) {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
    }

    pub(crate) fn finish_directory_stream(&self, session_id: &str, request_id: &str) {
        if let Ok(mut streams) = self.directory_streams.lock() {
            let is_active_request = streams
                .get(session_id)
                .map(|(active_request, _)| active_request == request_id)
                .unwrap_or(false);

            if is_active_request {
                streams.remove(session_id);
            }
        }
    }

    pub(crate) async fn get_system_info(
        &self,
        session_id: &str,
    ) -> Result<RemoteSystemSnapshot, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::GetSystemInfo { responder })
            .map_err(|_| not_accepting("systemInfo"))?;

        receiver
            .await
            .map_err(|_| request_canceled("systemInfo"))?
    }

    pub(crate) async fn create_directory(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::CreateDirectory { path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver
            .await
            .map_err(|_| request_canceled("createDirectory"))?
    }

    pub(crate) async fn create_file(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::CreateFile { path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("createFile"))?
    }

    pub(crate) async fn list_directory(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<Vec<RemoteFileEntry>, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ListDirectory { path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver
            .await
            .map_err(|_| request_canceled("listDirectory"))?
    }

    pub(crate) async fn read_file_text(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<RemoteFileContent, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ReadFileText { path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("readFile"))?
    }

    pub(crate) async fn write_file_text(
        &self,
        session_id: &str,
        path: String,
        content: String,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::WriteFileText {
                path,
                content,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("writeFile"))?
    }

    pub(crate) async fn rename_path(
        &self,
        session_id: &str,
        from_path: String,
        to_path: String,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::RenamePath {
                from_path,
                to_path,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("rename"))?
    }

    pub(crate) async fn delete_paths(
        &self,
        session_id: &str,
        entries: Vec<RemoteTransferEntry>,
        recursive: bool,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::DeletePaths {
                entries,
                recursive,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("delete"))?
    }

    pub(crate) fn start_upload(&self, request: RemoteUploadRequest) -> Result<(), AppError> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let sender = self.sender_for(&request.session_id)?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(task_id.clone(), Arc::clone(&cancellation));

        let send_result = sender
            .send(TerminalControl::StartUpload {
                request,
                cancellation,
            })
            .map_err(|_| not_accepting("upload"));

        if send_result.is_err() {
            self.transfer_cancellations
                .lock()
                .map_err(|_| lock_poisoned())?
                .remove(&task_id);
        }

        send_result
    }

    pub(crate) fn start_download(&self, request: RemoteDownloadRequest) -> Result<(), AppError> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let sender = self.sender_for(&request.session_id)?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(task_id.clone(), Arc::clone(&cancellation));

        let send_result = sender
            .send(TerminalControl::StartDownload {
                request,
                cancellation,
            })
            .map_err(|_| not_accepting("download"));

        if send_result.is_err() {
            self.transfer_cancellations
                .lock()
                .map_err(|_| lock_poisoned())?
                .remove(&task_id);
        }

        send_result
    }

    pub(crate) fn cancel_transfer(&self, task_id: &str) -> Result<(), AppError> {
        let cancellations = self
            .transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?;
        if let Some(cancellation) = cancellations.get(task_id) {
            cancellation.store(true, Ordering::SeqCst);
        }

        Ok(())
    }
}
