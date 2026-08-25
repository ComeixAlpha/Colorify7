use serde::Serialize;

/// 进度消息
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressMessage {
    pub stage: String,
    pub finished: bool,
    pub elapsed_ms: Option<u64>,
    pub output_dir: Option<String>,
    /// 失败信息（成功时为空）
    pub error: Option<String>,
}
