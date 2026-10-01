//! Unlock Music 解密核心。
//!
//! 这一层完全不依赖浏览器、GUI 或文件系统，只做「字节进、字节出」的解密，
//! 因此可以被 CLI、原生 GUI、以及编译成 WASM 复用到 Web 端。
//!
//! 移植来源：`src/decrypt-worker/`（React 版）。

pub mod audio_type;
pub mod buffer;
pub mod decipher;
pub mod error;

pub use decipher::{
    all_ciphers, decrypt_any, Decipher, DecipherOutcome, DecryptOptions, DecryptResult,
    KugouMusicDecipher, KuwoMusicDecipher, Migu3dDecipher, NetEaseCloudMusicDecipher,
    QQMusicV1Decipher, QQMusicV2Decipher, QingTingFmDecipher, TransparentDecipher, XiamiDecipher,
    XimalayaAndroidDecipher, XimalayaAndroidKind, XimalayaPCDecipher,
};
pub use error::UnsupportedSourceFile;
