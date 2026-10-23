use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::error::{
    codes, lock_poisoned, not_accepting, request_canceled, session_not_running,
    timeout as timeout_error, AppError,
};
use crate::models::{
    DockerContainerList, PartialFile, RemoteChmodRequest, RemoteCopyRequest, RemoteDeleteRequest, RemoteDownloadRequest, RemoteFileContent,
    RemoteFileEntry, RemoteSystemSnapshot, RemoteUploadRequest,
};
use crate::remote_exec::CommandTask;
use crate::sftp::transfer::TransferStop;

#[derive(Clone, Default)]
pub struct TerminalRegistry {
    sessions: Arc<Mutex<HashMap<String, TerminalHandle>>>,
    pub(crate) transfer_cancellations: Arc<Mutex<HashMap<String, TransferStop>>>,
    directory_streams: Arc<Mutex<HashMap<String, (String, Arc<AtomicBool>)>>>,
    // 删除、压缩解压、占用分析这些后台任务共用一张取消表，前端按任务 ID 取消
    file_operations: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

struct TerminalHandle {
    sender: mpsc::UnboundedSender<TerminalControl>,
    cancellation: CancellationToken,
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
    MeasureLatency {
        responder: oneshot::Sender<Result<u64, AppError>>,
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
    ResolveHome {
        responder: oneshot::Sender<Result<String, AppError>>,
    },
    StatPath {
        path: String,
        responder: oneshot::Sender<Result<RemoteFileEntry, AppError>>,
    },
    ResolveScopedPath {
        root: String,
        path: String,
        responder: oneshot::Sender<Result<RemoteFileEntry, AppError>>,
    },
    ListContainers {
        responder: oneshot::Sender<Result<DockerContainerList, AppError>>,
    },
    ForwardPort {
        remote_port: u16,
        responder: oneshot::Sender<Result<u16, AppError>>,
    },
    StopForward {
        local_port: u16,
    },
    ReadFileText {
        path: String,
        responder: oneshot::Sender<Result<RemoteFileContent, AppError>>,
    },
    WriteFileText {
        path: String,
        content: String,
        expected_version: String,
        backup_path: String,
        responder: oneshot::Sender<Result<String, AppError>>,
    },
    WriteFileBytes {
        path: String,
        content: Vec<u8>,
        overwrite: bool,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    RenamePath {
        from_path: String,
        to_path: String,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    CopyPath {
        from_path: String,
        to_path: String,
        recursive: bool,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    ChmodPath {
        path: String,
        mode: u32,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    DeletePaths {
        request: RemoteDeleteRequest,
        cancellation: CancellationToken,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    RunCommandTask {
        task: CommandTask,
        cancellation: CancellationToken,
        responder: oneshot::Sender<Result<(), AppError>>,
    },
    DiscardPartialFiles {
        files: Vec<PartialFile>,
        responder: oneshot::Sender<Result<Vec<PartialFile>, AppError>>,
    },
    StartUpload {
        request: RemoteUploadRequest,
        stop: TransferStop,
    },
    StartDownload {
        request: RemoteDownloadRequest,
        stop: TransferStop,
    },
    Close,
}

impl TerminalRegistry {
    pub(crate) fn replace(
        &self,
        session_id: String,
        sender: mpsc::UnboundedSender<TerminalControl>,
        cancellation: CancellationToken,
    ) -> Result<(), AppError> {
        self.cancel_directory_stream(&session_id);

        let mut sessions = self.sessions.lock().map_err(|_| lock_poisoned())?;
        if let Some(existing) = sessions.insert(session_id, TerminalHandle { sender, cancellation }) {
            existing.cancellation.cancel();
            let _ = existing.sender.send(TerminalControl::Close);
        }

        Ok(())
    }

    pub(crate) fn send(&self, session_id: &str, control: TerminalControl) -> Result<(), AppError> {
        let sessions = self.sessions.lock().map_err(|_| lock_poisoned())?;
        let sender = sessions.get(session_id).ok_or_else(session_not_running)?;

        sender.sender.send(control).map_err(|_| not_accepting("input"))
    }

    pub(crate) fn close(&self, session_id: &str) -> Result<(), AppError> {
        self.cancel_directory_stream(session_id);

        let sender = self
            .sessions
            .lock()
            .map_err(|_| lock_poisoned())?
            .remove(session_id);

        if let Some(sender) = sender {
            sender.cancellation.cancel();
            let _ = sender.sender.send(TerminalControl::Close);
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
            .map(|handle| handle.sender.clone())
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

    pub(crate) async fn measure_latency(&self, session_id: &str) -> Result<u64, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::MeasureLatency { responder })
            .map_err(|_| not_accepting("latency"))?;

        receiver
            .await
            .map_err(|_| request_canceled("latency"))?
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

    // 登录用户的家目录：终端提示符里的 ~ 要换算成绝对路径，文件面板才能跟过去
    pub(crate) async fn resolve_home(&self, session_id: &str) -> Result<String, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ResolveHome { responder })
            .map_err(|_| not_accepting("file"))?;

        receiver
            .await
            .map_err(|_| request_canceled("resolveHome"))?
    }

    // 终端里点开的路径是文件还是目录（跟随软链接）
    pub(crate) async fn stat_path(&self, session_id: &str, path: String) -> Result<RemoteFileEntry, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::StatPath { path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("statPath"))?
    }

    // 容器里的路径：在容器根下自己解析软链接，见 sftp::scoped_path
    pub(crate) async fn resolve_scoped_path(
        &self,
        session_id: &str,
        root: String,
        path: String,
    ) -> Result<RemoteFileEntry, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ResolveScopedPath { root, path, responder })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("resolvePath"))?
    }

    // 命令自带时限（remote_exec::docker），这里不再套一层
    pub(crate) async fn list_containers(&self, session_id: &str) -> Result<DockerContainerList, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ListContainers { responder })
            .map_err(|_| not_accepting("listContainers"))?;

        receiver.await.map_err(|_| request_canceled("listContainers"))?
    }

    // 在终端这条 SSH 连接上把服务器的 127.0.0.1:remote_port 转发到本机，返回本机端口
    pub(crate) async fn forward_port(&self, session_id: &str, remote_port: u16) -> Result<u16, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ForwardPort { remote_port, responder })
            .map_err(|_| not_accepting("forward"))?;

        receiver.await.map_err(|_| request_canceled("forwardPort"))?
    }

    // 会话已经断了就什么都不用做：转发随会话一起停了
    pub(crate) fn stop_forward(&self, session_id: &str, local_port: u16) -> Result<(), AppError> {
        match self.sender_for(session_id) {
            Ok(sender) => {
                let _ = sender.send(TerminalControl::StopForward { local_port });
                Ok(())
            }
            Err(_) => Ok(()),
        }
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
        expected_version: String,
        backup_path: String,
    ) -> Result<String, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::WriteFileText {
                path,
                content,
                expected_version,
                backup_path,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;

        receiver.await.map_err(|_| request_canceled("writeFile"))?
    }

    pub(crate) async fn write_file_bytes(
        &self,
        session_id: &str,
        path: String,
        content: Vec<u8>,
        overwrite: bool,
    ) -> Result<(), AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::WriteFileBytes { path, content, overwrite, responder })
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

    pub(crate) async fn copy_path(&self, request: RemoteCopyRequest) -> Result<(), AppError> {
        let sender = self.sender_for(&request.session_id)?;
        let (responder, receiver) = oneshot::channel();
        sender
            .send(TerminalControl::CopyPath {
                from_path: request.from_path,
                to_path: request.to_path,
                recursive: request.recursive,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;
        receiver.await.map_err(|_| request_canceled("copy"))?
    }

    pub(crate) async fn chmod_path(&self, request: RemoteChmodRequest) -> Result<(), AppError> {
        let sender = self.sender_for(&request.session_id)?;
        let (responder, receiver) = oneshot::channel();
        sender
            .send(TerminalControl::ChmodPath {
                path: request.path,
                mode: request.mode,
                responder,
            })
            .map_err(|_| not_accepting("file"))?;
        receiver.await.map_err(|_| request_canceled("chmod"))?
    }

    pub(crate) async fn start_delete(&self, request: RemoteDeleteRequest) -> Result<(), AppError> {
        let session_id = request.session_id.clone();
        let task_id = request.task_id.clone();
        self.start_file_operation(&session_id, &task_id, "delete", |cancellation, responder| {
            TerminalControl::DeletePaths { request, cancellation, responder }
        })
        .await
    }

    pub(crate) async fn start_command_task(&self, task: CommandTask) -> Result<(), AppError> {
        let session_id = task.session_id().to_string();
        let task_id = task.task_id().to_string();
        self.start_file_operation(&session_id, &task_id, "remoteCommand", |cancellation, responder| {
            TerminalControl::RunCommandTask { task, cancellation, responder }
        })
        .await
    }

    // 先登记任务再投递，取消、关会话和重连都能覆盖尚未启动的任务。
    async fn start_file_operation(
        &self,
        session_id: &str,
        task_id: &str,
        operation: &str,
        control: impl FnOnce(CancellationToken, oneshot::Sender<Result<(), AppError>>) -> TerminalControl,
    ) -> Result<(), AppError> {
        let (sender, cancellation) = {
            let sessions = self.sessions.lock().map_err(|_| lock_poisoned())?;
            let handle = sessions.get(session_id).ok_or_else(session_not_running)?;
            (handle.sender.clone(), handle.cancellation.child_token())
        };
        {
            let mut tasks = self.file_operations.lock().map_err(|_| lock_poisoned())?;
            if tasks.contains_key(task_id) {
                return Err(AppError::new(
                    codes::FILE_OPERATION_DUPLICATE,
                    "File operation is already running",
                ));
            }
            tasks.insert(task_id.to_string(), cancellation.clone());
        }
        let (responder, receiver) = oneshot::channel();
        let result = async {
            sender
                .send(control(cancellation.clone(), responder))
                .map_err(|_| not_accepting("file"))?;
            // 这里只限制任务接收确认，不限制任务总时长；超时后接收端消失，任务不得启动。
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(AppError::new(
                    codes::FILE_OPERATION_CANCELED,
                    "File operation was canceled",
                )),
                received = timeout(Duration::from_secs(12), receiver) => {
                    received.map_err(|_| timeout_error(operation, 12))?
                        .map_err(|_| request_canceled(operation))?
                }
            }
        }
        .await;
        if result.is_err() {
            cancellation.cancel();
            self.finish_file_operation(task_id);
        }
        result
    }

    pub(crate) fn cancel_file_operation(&self, task_id: &str) -> Result<(), AppError> {
        let tasks = self.file_operations.lock().map_err(|_| lock_poisoned())?;
        if let Some(cancellation) = tasks.get(task_id) {
            cancellation.cancel();
        }
        Ok(())
    }

    pub(crate) async fn discard_partial_files(
        &self,
        session_id: &str,
        files: Vec<PartialFile>,
    ) -> Result<Vec<PartialFile>, AppError> {
        let sender = self.sender_for(session_id)?;
        let (responder, receiver) = oneshot::channel();
        sender
            .send(TerminalControl::DiscardPartialFiles { files, responder })
            .map_err(|_| not_accepting("file"))?;
        receiver.await.map_err(|_| request_canceled("discardPartialFiles"))?
    }

    pub(crate) fn finish_file_operation(&self, task_id: &str) {
        if let Ok(mut tasks) = self.file_operations.lock() {
            tasks.remove(task_id);
        }
    }

    pub(crate) fn start_upload(&self, request: RemoteUploadRequest) -> Result<(), AppError> {
        let stop = TransferStop::default();
        let sender = self.sender_for(&request.session_id)?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(task_id.clone(), stop.clone());

        let send_result = sender
            .send(TerminalControl::StartUpload { request, stop })
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
        let stop = TransferStop::default();
        let sender = self.sender_for(&request.session_id)?;
        let task_id = request.task_id.clone();

        self.transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(task_id.clone(), stop.clone());

        let send_result = sender
            .send(TerminalControl::StartDownload { request, stop })
            .map_err(|_| not_accepting("download"));

        if send_result.is_err() {
            self.transfer_cancellations
                .lock()
                .map_err(|_| lock_poisoned())?
                .remove(&task_id);
        }

        send_result
    }

    // pause = true 停下后保留半成品供续传，false 当作取消删掉
    pub(crate) fn stop_transfer(&self, task_id: &str, pause: bool) -> Result<(), AppError> {
        let stops = self
            .transfer_cancellations
            .lock()
            .map_err(|_| lock_poisoned())?;
        if let Some(stop) = stops.get(task_id) {
            stop.request(pause);
        }

        Ok(())
    }
}
