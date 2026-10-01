use std::fmt;

/// 文件不被任何解密器识别或无法解出有效音频。
///
/// 对应 TS 侧的 `UnsupportedSourceFile`（`src/decrypt-worker/util/DecryptError.ts`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSourceFile {
    message: String,
}

impl UnsupportedSourceFile {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for UnsupportedSourceFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "无法识别的加密文件：{}", self.message)
    }
}

impl std::error::Error for UnsupportedSourceFile {}
