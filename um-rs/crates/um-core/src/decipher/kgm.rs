use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_kgm::header::Header;
use umc_kgm::Decipher as KugouDecipher;

/// 解析头部时读取的长度，与 TS 侧 `buffer.subarray(0, 0x400)` 一致。
const HEADER_READ_LEN: usize = 0x400;

/// 酷狗 KGM / VPR 解密器（`.kgm` / `.vpr` / `.kgg`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/KugouMusic.ts`。
/// V5 之外的版本不需要密钥；V5 必须由调用方提供 ekey。
pub struct KugouMusicDecipher;

impl Decipher for KugouMusicDecipher {
    fn cipher_name(&self) -> &'static str {
        "Kugou"
    }

    fn decrypt(&self, buffer: &[u8], options: &DecryptOptions) -> DecipherOutcome {
        let header_len = HEADER_READ_LEN.min(buffer.len());
        let header = match Header::from_buffer(&buffer[..header_len]) {
            Ok(header) => header,
            // magic 不匹配即「不是酷狗文件」
            Err(_) => return DecipherOutcome::NotThisCipher,
        };

        // 构造时会用 header 里的 challenge data 做自检，失败说明密钥不对或版本不支持
        let decipher = match KugouDecipher::new_v5(&header, options.kugou_key.clone()) {
            Ok(decipher) => decipher,
            Err(err) => return DecipherOutcome::Failed(err.to_string()),
        };

        // TS 侧硬编码从 0x400 开始，这里改用 header 里记录的偏移量（正常文件两者相同）
        let start = header.offset_to_data.min(buffer.len());
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

    const HDR_V2: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/kgm/src/__fixtures__/kgm_v2_hdr.bin");
    const HDR_V3: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/kgm/src/__fixtures__/kgm_v3_hdr.bin");
    const HDR_V5: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/kgm/src/__fixtures__/kgm_v5_hdr.bin");
    const HDR_INVALID_MAGIC: &[u8] = include_bytes!(
        "../../../../vendor/lib_um_crypto_rust/um_crypto/kgm/src/__fixtures__/kgm_invalid_magic.bin"
    );

    #[test]
    fn rejects_invalid_magic() {
        let outcome = KugouMusicDecipher.decrypt(HDR_INVALID_MAGIC, &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn parses_v2_and_v3_headers_instead_of_rejecting_them() {
        // 官方样本只包含头部，没有音频数据，因此不可能解出音频。
        // 关键是不能被判成「不是酷狗文件」——说明头部解析与版本分派都走通了。
        //
        // 两种可接受结果：
        //   - Ok 且数据为空：offset_to_data(0x400) 超出样本长度，音频区为空，
        //     顺带验证了取偏移量时的钳制不会 panic；
        //   - Failed 且为自检失败：该样本的自检数据无法通过校验（真实文件才能通过）。
        for fixture in [HDR_V2, HDR_V3] {
            match KugouMusicDecipher.decrypt(fixture, &DecryptOptions::default()) {
                DecipherOutcome::Ok(done) => {
                    assert!(done.data.is_empty(), "头部样本里不应有音频数据");
                }
                DecipherOutcome::Failed(message) => assert!(
                    message.contains("self-test failed"),
                    "只允许自检失败，实际为：{message}"
                ),
                DecipherOutcome::NotThisCipher => {
                    panic!("不应把官方头部样本判成「不是酷狗文件」")
                }
            }
        }
    }

    #[test]
    fn v5_requires_ekey() {
        let outcome = KugouMusicDecipher.decrypt(HDR_V5, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert!(message.contains("V5 requires ekey")),
            other => panic!("应当提示缺少 ekey，实际为 {other:?}"),
        }
    }

    #[test]
    fn rejects_truncated_data() {
        let outcome = KugouMusicDecipher.decrypt(&[0u8; 8], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }
}
