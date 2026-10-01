use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_kuwo::{Header, DATA_START_OFFSET};

/// 解析头部时读取的长度，与 TS 侧 `buffer.subarray(0, 0x400)` 一致。
/// 头部结构实际只占 0x3C 字节。
const HEADER_READ_LEN: usize = 0x400;

/// 酷我音乐 KWM 解密器（`.kwm`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/KuwoMusic.ts`。
/// V1 由 resource id 推导密钥，V2 需要调用方提供 ekey。
pub struct KuwoMusicDecipher;

impl Decipher for KuwoMusicDecipher {
    fn cipher_name(&self) -> &'static str {
        "Kuwo"
    }

    fn decrypt(&self, buffer: &[u8], options: &DecryptOptions) -> DecipherOutcome {
        let header_len = HEADER_READ_LEN.min(buffer.len());
        let header = match Header::from_bytes(&buffer[..header_len]) {
            Ok(header) => header,
            Err(_) => return DecipherOutcome::NotThisCipher,
        };

        let decipher = match umc_kuwo::Decipher::new(&header, options.kwm2_key.as_deref()) {
            Ok(decipher) => decipher,
            Err(err) => return DecipherOutcome::Failed(err.to_string()),
        };

        let start = DATA_START_OFFSET.min(buffer.len());
        let mut audio = buffer[start..].to_vec();
        for (block, offset) in chunk_buffer(&mut audio, DEFAULT_BLOCK_LEN) {
            decipher.decrypt(block, offset);
        }

        DecipherOutcome::Ok(Box::new(Deciphered {
            data: audio,
            override_extension: None,
            cipher_name: self.cipher_name(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_kuwo_magic() {
        let outcome = KuwoMusicDecipher.decrypt(&[0u8; 0x400], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn rejects_truncated_header() {
        let outcome = KuwoMusicDecipher.decrypt(b"yeelion-kuwo-tme", &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn v2_requires_ekey() {
        // version = 2 的头部缺少 ekey 时应报错，而不是被当成「不是酷我文件」
        let mut header = vec![0u8; 0x400];
        header[..16].copy_from_slice(b"yeelion-kuwo-tme");
        header[0x10..0x14].copy_from_slice(&2u32.to_le_bytes());

        let outcome = KuwoMusicDecipher.decrypt(&header, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert!(message.contains("EKey required")),
            other => panic!("应当提示缺少 ekey，实际为 {other:?}"),
        }
    }

    #[test]
    fn unsupported_version_is_reported() {
        let mut header = vec![0u8; 0x400];
        header[..16].copy_from_slice(b"yeelion-kuwo\0\0\0\0");
        header[0x10..0x14].copy_from_slice(&9u32.to_le_bytes());

        let outcome = KuwoMusicDecipher.decrypt(&header, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert!(message.contains("Unsupported version")),
            other => panic!("应当提示版本不支持，实际为 {other:?}"),
        }
    }
}
