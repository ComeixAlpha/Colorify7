use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};

/// 一个已连接的频道
struct Channel {
    id: u64,
    tx: mpsc::UnboundedSender<Message>,
}

/// WebSocket 服务器（Clone 共享）
#[derive(Clone)]
pub struct WsServer {
    port: u16,
    channels: Arc<RwLock<Vec<Channel>>>,
    next_id: Arc<AtomicU64>,
    on_message: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl WsServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            channels: Arc::new(RwLock::new(Vec::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            on_message: None,
        }
    }

    pub fn with_on_message(port: u16, on_message: impl Fn(String) + Send + Sync + 'static) -> Self {
        Self {
            on_message: Some(Arc::new(on_message)),
            ..Self::new(port)
        }
    }

    #[allow(dead_code)]
    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn connection_count(&self) -> usize {
        self.channels.read().await.len()
    }

    pub async fn broadcast_command(&self, command: &str) {
        let msg = Message::text(Datapack::command_request(command, None));
        let mut guard = self.channels.write().await;
        guard.retain(|ch| ch.tx.send(msg.clone()).is_ok());
    }

    pub async fn bind(&self) -> std::io::Result<TcpListener> {
        TcpListener::bind(("127.0.0.1", self.port)).await
    }

    pub async fn serve_with(self, listener: TcpListener) {
        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => continue,
            };
            let this = self.clone();
            tokio::spawn(async move {
                this.handle_conn(stream).await;
            });
        }
    }

    pub async fn close_channels(&self) {
        self.channels.write().await.clear();
    }

    async fn handle_conn(&self, stream: TcpStream) {
        let upgraded = accept_hdr_async(stream, |req: &Request, mut resp: Response| {
            let is_ws = req
                .headers()
                .get("upgrade")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
            if is_ws {
                Ok(resp)
            } else {
                *resp.status_mut() = StatusCode::NOT_FOUND;
                Ok(resp)
            }
        })
        .await;

        match upgraded {
            Ok(ws) => self.handle_ws(ws).await,
            Err(_) => {}
        }
    }

    async fn handle_ws(&self, ws: WebSocketStream<TcpStream>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        self.channels.write().await.push(Channel { id, tx });

        // 写任务
        let writer = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // 读循环
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Some(cb) = &self.on_message {
                        cb(text.to_string());
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }

        // 断开
        drop(stream);
        self.channels.write().await.retain(|c| c.id != id);
        writer.abort();
    }
}

pub struct Datapack;

impl Datapack {
    const DEFAULT_UUID: &'static str = "00000000-0000-0000-0000-000000000000";

    fn header(purpose: &str, uuid: Option<&str>) -> Value {
        json!({
            "requestId": uuid.unwrap_or(Self::DEFAULT_UUID),
            "messagePurpose": purpose,
            "version": 1,
            "messageType": "commandRequest",
        })
    }

    /// 订阅事件
    #[allow(dead_code)]
    pub fn subscribe(event_name: &str, uuid: Option<&str>) -> String {
        json!({
            "body": { "eventName": event_name },
            "header": Self::header("subscribe", uuid),
        })
        .to_string()
    }

    #[allow(dead_code)]
    pub fn unsubscribe(event_name: &str, uuid: Option<&str>) -> String {
        json!({
            "body": { "eventName": event_name },
            "header": Self::header("unsubscribe", uuid),
        })
        .to_string()
    }

    /// 命令请求
    pub fn command_request(command: &str, uuid: Option<&str>) -> String {
        json!({
            "body": { "commandLine": command },
            "header": Self::header("commandRequest", uuid),
        })
        .to_string()
    }
}

use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{Emitter, State};

/// 任务状态
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Idle,
    Running,
    Paused,
}

/// 任务进度快照
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub state: TaskState,
    pub sent: usize,
    pub total: usize,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self {
            state: TaskState::Idle,
            sent: 0,
            total: 0,
        }
    }
}

/// 任务
struct WsTask {
    commands: Vec<String>,
    sent: AtomicUsize,
    state: Mutex<TaskState>,
}

impl WsTask {
    fn new(commands: Vec<String>) -> Self {
        Self {
            commands,
            sent: AtomicUsize::new(0),
            state: Mutex::new(TaskState::Running),
        }
    }

    fn sent(&self) -> usize {
        self.sent.load(Ordering::Relaxed)
    }

    fn total(&self) -> usize {
        self.commands.len()
    }

    fn state(&self) -> TaskState {
        *self.state.lock().unwrap()
    }

    fn snapshot(&self) -> TaskStatus {
        TaskStatus {
            state: self.state(),
            sent: self.sent(),
            total: self.total(),
        }
    }
}

fn parse_position(msg: &str) -> Option<[i32; 3]> {
    let v: Value = serde_json::from_str(msg).ok()?;
    let pos = v.get("body")?.get("position")?;
    Some([
        pos.get("x")?.as_i64()? as i32,
        pos.get("y")?.as_i64()? as i32,
        pos.get("z")?.as_i64()? as i32,
    ])
}

/// WebSocket 服务器状态
pub struct WsState {
    server: Mutex<Option<WsServer>>,
    handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    port: Mutex<u16>,
    task: Mutex<Option<Arc<WsTask>>>,
    task_handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    execute_loc: Arc<Mutex<Option<[i32; 3]>>>,
}

impl Default for WsState {
    fn default() -> Self {
        Self {
            server: Mutex::new(None),
            handle: Mutex::new(None),
            port: Mutex::new(8080),
            task: Mutex::new(None),
            task_handle: Mutex::new(None),
            execute_loc: Arc::new(Mutex::new(None)),
        }
    }
}

impl WsState {
    async fn status(&self) -> Result<WsStatus, String> {
        let running = self.server.lock().unwrap().is_some();
        let server = self.server.lock().unwrap().clone();
        let connections = match server {
            Some(s) => s.connection_count().await as u64,
            None => 0,
        };
        Ok(WsStatus {
            running,
            port: *self.port.lock().unwrap(),
            connections,
        })
    }

    /// 启动服务器
    async fn launch(&self, app: tauri::AppHandle, port: u16) -> Result<WsStatus, String> {
        if self.server.lock().unwrap().is_some() {
            return Err("WebSocket 服务器已在运行".into());
        }
        let loc = self.execute_loc.clone();
        let server = WsServer::with_on_message(port, move |msg| {
            let _ = app.emit("ws-message", &msg);
            // 记录玩家位置
            if let Some(pos) = parse_position(&msg) {
                *loc.lock().unwrap() = Some(pos);
            }
        });
        // 预检端口占用
        let listener = server
            .bind()
            .await
            .map_err(|e| format!("端口 {port} 绑定失败: {e}"))?;
        let server2 = server.clone();
        let handle = tauri::async_runtime::spawn(async move {
            server2.serve_with(listener).await;
        });
        *self.port.lock().unwrap() = port;
        *self.server.lock().unwrap() = Some(server);
        *self.handle.lock().unwrap() = Some(handle);
        self.status().await
    }

    async fn close(&self) -> Result<(), String> {
        let server = { self.server.lock().unwrap().take() };
        if let Some(s) = server {
            s.close_channels().await;
        }
        let handle = { self.handle.lock().unwrap().take() };
        if let Some(h) = handle {
            h.abort();
        }
        // 关闭服务器时一并终止任务
        let _ = { self.task.lock().unwrap().take() };
        let task_handle = { self.task_handle.lock().unwrap().take() };
        if let Some(h) = task_handle {
            h.abort();
        }
        Ok(())
    }

    async fn broadcast(&self, command: &str) -> Result<u64, String> {
        let server = self.server.lock().unwrap().clone();
        match server {
            Some(s) => {
                s.broadcast_command(command).await;
                Ok(s.connection_count().await as u64)
            }
            None => Err("WebSocket 服务器未启动".into()),
        }
    }

    /// 启动命令发送任务
    pub async fn task_start(
        &self,
        commands: Vec<String>,
        delay_ms: u64,
    ) -> Result<TaskStatus, String> {
        let server = self
            .server
            .lock()
            .unwrap()
            .clone()
            .ok_or("WebSocket 服务器未启动")?;
        if commands.is_empty() {
            return Err("命令列表为空".into());
        }
        let delay = delay_ms.max(1);
        // 终止旧任务
        if let Some(h) = self.task_handle.lock().unwrap().take() {
            h.abort();
        }
        // 每次任务重新定位
        *self.execute_loc.lock().unwrap() = None;
        let task = Arc::new(WsTask::new(commands));
        let task2 = task.clone();
        let loc = self.execute_loc.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let total = task2.total();
            let mut i = 0usize;

            server.broadcast_command("testforblock ~ ~ ~ air").await;
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while loc.lock().unwrap().is_none() && std::time::Instant::now() < deadline {
                if task2.state() == TaskState::Idle {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let base = loc.lock().unwrap().clone();

            // 计时起点
            let task_start = std::time::Instant::now();

            while i < total {
                loop {
                    match task2.state() {
                        TaskState::Paused => tokio::time::sleep(Duration::from_millis(50)).await,
                        TaskState::Idle => return,
                        TaskState::Running => break,
                    }
                }
                tokio::time::sleep(Duration::from_millis(delay)).await;
                if task2.state() != TaskState::Running {
                    continue;
                }

                let cmd = match base {
                    Some([x, y, z]) => {
                        format!("execute positioned {x} {y} {z} run {}", task2.commands[i])
                    }
                    None => task2.commands[i].clone(),
                };
                server.broadcast_command(&cmd).await;
                i += 1;
                task2.sent.store(i, Ordering::Relaxed);

                // 进度提示
                if base.is_some() && (i % 25 == 0 || i == total) {
                    let title = format!(
                        "title @s actionbar §bColorify v7 - ComeixAlpha§f: §f{i} / {total}"
                    );
                    server.broadcast_command(&title).await;
                }
            }
            // 发送完毕
            let elapsed = task_start.elapsed().as_secs_f32();
            let done = format!(
                "tellraw @s {}",
                json!({
                    "rawtext": [
                        { "text": "[§bColorify§f]: §a完成! §f用时 " },
                        { "text": format!("{elapsed:.2} s") }
                    ]
                })
            );
            server.broadcast_command(&done).await;
            *task2.state.lock().unwrap() = TaskState::Idle;
        });
        *self.task_handle.lock().unwrap() = Some(handle);
        *self.task.lock().unwrap() = Some(task);
        self.task_status().await
    }

    async fn task_pause(&self) -> Result<TaskStatus, String> {
        let task = self
            .task
            .lock()
            .unwrap()
            .clone()
            .ok_or("没有进行中的任务")?;
        *task.state.lock().unwrap() = TaskState::Paused;
        Ok(task.snapshot())
    }

    async fn task_resume(&self) -> Result<TaskStatus, String> {
        let task = self
            .task
            .lock()
            .unwrap()
            .clone()
            .ok_or("没有进行中的任务")?;
        *task.state.lock().unwrap() = TaskState::Running;
        Ok(task.snapshot())
    }

    async fn task_stop(&self) -> Result<TaskStatus, String> {
        let task = self
            .task
            .lock()
            .unwrap()
            .clone()
            .ok_or("没有进行中的任务")?;
        *task.state.lock().unwrap() = TaskState::Idle;
        if let Some(h) = self.task_handle.lock().unwrap().take() {
            h.abort();
        }
        Ok(task.snapshot())
    }

    async fn task_status(&self) -> Result<TaskStatus, String> {
        Ok(match &*self.task.lock().unwrap() {
            Some(t) => t.snapshot(),
            None => TaskStatus::default(),
        })
    }
}

/// WebSocket 服务器状态快照
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsStatus {
    running: bool,
    port: u16,
    connections: u64,
}

/// 启动 WebSocket 服务器
#[tauri::command]
pub async fn ws_launch(
    app: tauri::AppHandle,
    state: State<'_, Arc<WsState>>,
    port: Option<u16>,
) -> Result<WsStatus, String> {
    state.launch(app, port.unwrap_or(8080)).await
}

/// 停止 WebSocket 服务器并断开所有连接
#[tauri::command]
pub async fn ws_close(state: State<'_, Arc<WsState>>) -> Result<(), String> {
    state.close().await
}

/// 广播一条命令给所有已连接频道
#[tauri::command]
pub async fn ws_broadcast(state: State<'_, Arc<WsState>>, command: String) -> Result<u64, String> {
    state.broadcast(&command).await
}

/// 查询服务器状态
#[tauri::command]
pub async fn ws_status(state: State<'_, Arc<WsState>>) -> Result<WsStatus, String> {
    state.status().await
}

/// 启动命令发送任务
#[tauri::command]
pub async fn ws_task_start(
    state: State<'_, Arc<WsState>>,
    commands: Vec<String>,
    delay: Option<u64>,
) -> Result<TaskStatus, String> {
    state.task_start(commands, delay.unwrap_or(10)).await
}

/// 暂停当前任务
#[tauri::command]
pub async fn ws_task_pause(state: State<'_, Arc<WsState>>) -> Result<TaskStatus, String> {
    state.task_pause().await
}

/// 继续被暂停的任务
#[tauri::command]
pub async fn ws_task_resume(state: State<'_, Arc<WsState>>) -> Result<TaskStatus, String> {
    state.task_resume().await
}

/// 结束当前任务
#[tauri::command]
pub async fn ws_task_stop(state: State<'_, Arc<WsState>>) -> Result<TaskStatus, String> {
    state.task_stop().await
}

/// 查询任务进度
#[tauri::command]
pub async fn ws_task_status(state: State<'_, Arc<WsState>>) -> Result<TaskStatus, String> {
    state.task_status().await
}
