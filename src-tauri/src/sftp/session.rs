use std::{future::Future, sync::Arc};

use russh::client;
use russh_sftp::client::SftpSession;
use tokio::sync::{oneshot, Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::error::{codes, request_canceled, AppError};
use crate::sftp::open_sftp_session;
use crate::ssh::client::ConnectionTestClient;

#[derive(Clone)]
pub(crate) struct CachedSftp {
    session: Arc<Mutex<client::Handle<ConnectionTestClient>>>,
    cached: Arc<Mutex<Option<Arc<SftpSession>>>>,
    permits: Arc<Semaphore>,
    mutations: Arc<Semaphore>,
    cancellation: CancellationToken,
}

impl CachedSftp {
    pub(crate) fn new(
        session: Arc<Mutex<client::Handle<ConnectionTestClient>>>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            session,
            cached: Arc::new(Mutex::new(None)),
            permits: Arc::new(Semaphore::new(4)),
            mutations: Arc::new(Semaphore::new(1)),
            cancellation,
        }
    }

    // 缓存锁只保护建连；具体请求依靠 SFTP request id 复用，慢读取不阻塞目录操作。
    async fn get(&self) -> Result<Arc<SftpSession>, AppError> {
        let mut cached = self.cached.lock().await;
        if let Some(sftp) = cached.as_ref() {
            return Ok(Arc::clone(sftp));
        }
        let sftp = Arc::new(open_sftp_session(Arc::clone(&self.session)).await?);
        *cached = Some(Arc::clone(&sftp));
        Ok(sftp)
    }

    // 超时会丢弃 oneshot 接收端，必须同时丢弃操作 future，阻止后台继续发出修改请求。
    pub(crate) fn spawn<T, F, Fut>(
        &self,
        mut responder: oneshot::Sender<Result<T, AppError>>,
        mutation: bool,
        operation: F,
    ) where
        T: Send + 'static,
        F: FnOnce(Arc<SftpSession>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        let cache = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut active_sftp = None;
            let result = tokio::select! {
                biased;
                _ = responder.closed() => None,
                _ = cache.cancellation.cancelled() => Some(Err(request_canceled("file"))),
                result = cache.run(mutation, &mut active_sftp, operation) => Some(result),
            };
            let invalidated = result.is_none()
                || cache.cancellation.is_cancelled()
                || result.as_ref().and_then(|result| result.as_ref().err()).is_some_and(|error| {
                    matches!(
                        error.code,
                        codes::SSH_CONNECTION | codes::SSH_DISCONNECTED | codes::IO_FAILED
                    )
                });
            if let Some(result) = result {
                let _ = responder.send(result);
            }
            // 取消的 read_dir 可能来不及关闭远端句柄；最后一个在途请求结束后释放旧通道。
            if invalidated {
                if let Some(sftp) = active_sftp {
                    cache.invalidate(&sftp).await;
                }
            }
        });
    }

    async fn run<T, F, Fut>(
        &self,
        mutation: bool,
        active_sftp: &mut Option<Arc<SftpSession>>,
        operation: F,
    ) -> Result<T, AppError>
    where
        F: FnOnce(Arc<SftpSession>) -> Fut,
        Fut: Future<Output = Result<T, AppError>>,
    {
        // 修改保持串行，但不占住读取通道；等待修改锁时不消耗四个并发名额。
        let _mutation = if mutation {
            Some(
                self.mutations
                    .acquire()
                    .await
                    .map_err(|_| request_canceled("file"))?,
            )
        } else {
            None
        };
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| request_canceled("file"))?;
        let sftp = self.get().await?;
        *active_sftp = Some(Arc::clone(&sftp));
        operation(sftp).await
    }

    async fn invalidate(&self, sftp: &Arc<SftpSession>) {
        let mut cached = self.cached.lock().await;
        // 旧请求出错不能清掉另一请求刚建的新通道，也不能关闭其他在途请求的通道。
        if cached
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, sftp))
        {
            *cached = None;
        }
    }

    pub(crate) async fn close(&self) {
        self.cancellation.cancel();
        let sftp = self.cached.lock().await.take();
        if let Some(sftp) = sftp {
            let _ = sftp.close().await;
        }
    }
}
