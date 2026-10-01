//! 音频类型探测。
//!
//! 对应 TS 侧 `src/decrypt-worker/util/audioType.ts`。
//! 实际探测逻辑委托给官方 `um_audio` crate，这里只负责复现原版
//! `detectAudioExtension` / `isDataLooksLikeAudio` 的调用方式。

pub use um_audio::AudioType;

use um_audio::{detect_audio_type, AudioError, AudioType as Detected};

/// 首次读取的字节数，对应 TS 侧的 `neededLength = 0x100`。
pub const INITIAL_READ_LEN: usize = 0x100;

/// 探测文件应使用的扩展名，无法识别时为 `"bin"`。
///
/// 移植自 TS 侧 `detectAudioExtension`：从 0x100 字节开始尝试，
/// 当探测结果表示「需要更多头部」时，按其给出的绝对长度重新探测。
///
/// ⚠️ 与原版的一处差异：原版在遇到被截断的文件时（`needMore` 始终大于
/// 缓冲区长度）会陷入死循环，这里改为返回 `"bin"` 直接终止。
pub fn detect_audio_extension(buffer: &[u8]) -> String {
    let mut needed = INITIAL_READ_LEN;

    loop {
        let end = needed.min(buffer.len());
        let (need_more, audio_type) = match detect_audio_type(&buffer[..end]) {
            Ok(detected) => (0usize, detected.as_str().to_owned()),
            // WASM 层同样把 NeedMoreHeader 映射成 needMore + "bin"
            Err(AudioError::NeedMoreHeader(required)) => (required, Detected::Unknown.as_str().to_owned()),
        };

        // need_more 为 0 表示已可判定；已经没有更多字节可读时也必须终止
        if need_more == 0 || end >= buffer.len() {
            return audio_type;
        }
        needed = need_more;
    }
}

/// 判断头部数据看起来像不像音频。
///
/// 移植自 TS 侧 `isDataLooksLikeAudio`：`needMore != 0` 说明命中了有效头部
/// （例如 ID3），同样视为音频。
pub fn is_data_looks_like_audio(buffer: &[u8]) -> bool {
    if buffer.len() < 0x20 {
        return false;
    }

    match detect_audio_type(&buffer[..0x20]) {
        Ok(detected) => detected != Detected::Unknown,
        Err(AudioError::NeedMoreHeader(_)) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 官方 um_audio 自带的样本，用于验证探测结果与上游一致。
    const MP3_WITH_ID3V2: &[u8] = include_bytes!(
        "../../../vendor/lib_um_crypto_rust/um_audio/src/__fixtures__/mp3_with_id3v2.bin"
    );

    /// 构造一个足够长的缓冲区：探测器要求至少能读到 0x10 字节魔数，
    /// 无 ID3 时还会要求 32 字节以上。
    fn padded(magic: &[u8]) -> Vec<u8> {
        let mut buffer = vec![0u8; 64];
        buffer[..magic.len()].copy_from_slice(magic);
        buffer
    }

    #[test]
    fn detects_real_mp3_fixture_with_id3_tag() {
        assert_eq!(detect_audio_extension(MP3_WITH_ID3V2), "mp3");
    }

    #[test]
    fn detects_common_containers() {
        assert_eq!(detect_audio_extension(&padded(b"fLaC")), "flac");
        assert_eq!(detect_audio_extension(&padded(b"OggS")), "ogg");
        // DFF 的魔数是 FRM8，不是 "DSD "
        assert_eq!(detect_audio_extension(&padded(b"FRM8")), "dff");
        assert_eq!(detect_audio_extension(&padded(b"MAC ")), "ape");
        assert_eq!(detect_audio_extension(&padded(&[0x1A, 0x45, 0xDF, 0xA3])), "mka");

        let mut wav = padded(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        assert_eq!(detect_audio_extension(&wav), "wav");

        let wma = padded(&[0x30, 0x26, 0xB2, 0x75]);
        assert_eq!(detect_audio_extension(&wma), "wma");
    }

    #[test]
    fn detects_mp4_container_by_ftyp_brand() {
        let mut m4a = vec![0u8; 64];
        m4a[..12].copy_from_slice(b"\x00\x00\x00\x20ftypM4A ");
        assert_eq!(detect_audio_extension(&m4a), "m4a");

        let mut mp4 = vec![0u8; 64];
        mp4[..12].copy_from_slice(b"\x00\x00\x00\x20ftypisom");
        assert_eq!(detect_audio_extension(&mp4), "mp4");
    }

    #[test]
    fn unknown_data_falls_back_to_bin() {
        assert_eq!(detect_audio_extension(&[0u8; 64]), "bin");
        assert_eq!(detect_audio_extension(&[]), "bin");
        // 缓冲区被截断时不能死循环
        assert_eq!(detect_audio_extension(b"ID3"), "bin");
    }

    #[test]
    fn too_short_data_is_not_audio() {
        assert!(!is_data_looks_like_audio(b"ID3"));
    }

    #[test]
    fn mp3_header_is_recognized_as_audio() {
        assert!(is_data_looks_like_audio(MP3_WITH_ID3V2));
    }
}
