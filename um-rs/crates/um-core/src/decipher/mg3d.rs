use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};

/// 猜测密钥时读取的头部长度，与 TS 侧 `buffer.subarray(0, 0x100)` 一致。
const HEADER_LEN: usize = 0x100;

/// 咪咕 3D 音乐解密器（`.mg3d`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/Migu3d.ts` 的 `Migu3DKeylessDecipher`。
/// 该格式没有魔数，只能从头部反推密钥；推不出来就认为不是本格式。
pub struct Migu3dDecipher;

impl Decipher for Migu3dDecipher {
    fn cipher_name(&self) -> &'static str {
        "Migu3D (Keyless)"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        if buffer.len() < HEADER_LEN {
            return DecipherOutcome::NotThisCipher;
        }

        let key = match umc_mg3d::guess_key(&buffer[..HEADER_LEN]) {
            Some(key) => key,
            None => return DecipherOutcome::NotThisCipher,
        };

        let decipher = match umc_mg3d::Decipher::new_from_final_key(&key) {
            Ok(decipher) => decipher,
            Err(err) => return DecipherOutcome::Failed(err.to_string()),
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_input() {
        let outcome = Migu3dDecipher.decrypt(&[0u8; 32], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn rejects_data_without_guessable_key() {
        let outcome = Migu3dDecipher.decrypt(&[0u8; 512], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }
}
