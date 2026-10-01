use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_qtfm::nonce::make_decipher_iv;
use umc_qtfm::Decipher as QtfmDecipher;

/// 设备密钥长度（AES-128）。
const KEY_LEN: usize = 0x10;

/// 蜻蜓 FM 解密器（`.qta`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/QingTingFM.ts`。
/// 密钥来自设备信息（十六进制），IV 由文件名推导，两者缺一不可。
pub struct QingTingFmDecipher;

impl Decipher for QingTingFmDecipher {
    fn cipher_name(&self) -> &'static str {
        "QingTingFM (Android, qta)"
    }

    fn decrypt(&self, buffer: &[u8], options: &DecryptOptions) -> DecipherOutcome {
        // 未提供密钥时与原版一致：按空密钥处理，最终报同一个错误
        let key_hex = options.qingting_android_key.as_deref().unwrap_or_default();
        let key = hex::decode(key_hex).unwrap_or_default();

        let iv = match make_decipher_iv(&options.file_name) {
            Ok(iv) => iv,
            Err(_) => return DecipherOutcome::Failed(invalid_key_message()),
        };

        if key.len() != KEY_LEN {
            return DecipherOutcome::Failed(invalid_key_message());
        }
        let device_key = device_key_from(&key);

        let decipher = QtfmDecipher::new(&device_key, &iv);
        let mut audio = buffer.to_vec();
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

fn invalid_key_message() -> String {
    "device key or iv invalid".to_owned()
}

fn device_key_from(key: &[u8]) -> [u8; KEY_LEN] {
    let mut result = [0u8; KEY_LEN];
    result.copy_from_slice(key);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_invalid_key_when_missing() {
        let outcome = QingTingFmDecipher.decrypt(&[0u8; 64], &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert_eq!(message, "device key or iv invalid"),
            other => panic!("应当提示密钥无效，实际为 {other:?}"),
        }
    }

    #[test]
    fn reports_invalid_key_when_length_is_wrong() {
        let options = DecryptOptions {
            file_name: "song.qta".to_owned(),
            qingting_android_key: Some("00112233".to_owned()),
            ..DecryptOptions::default()
        };

        let outcome = QingTingFmDecipher.decrypt(&[0u8; 64], &options);
        match outcome {
            DecipherOutcome::Failed(message) => assert_eq!(message, "device key or iv invalid"),
            other => panic!("应当提示密钥无效，实际为 {other:?}"),
        }
    }

    #[test]
    fn reports_invalid_key_when_hex_is_malformed() {
        let options = DecryptOptions {
            file_name: "song.qta".to_owned(),
            qingting_android_key: Some("not-hex".to_owned()),
            ..DecryptOptions::default()
        };

        let outcome = QingTingFmDecipher.decrypt(&[0u8; 64], &options);
        assert!(matches!(outcome, DecipherOutcome::Failed(_)));
    }
}
