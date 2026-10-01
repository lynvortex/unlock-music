/// 解密时的分块大小，与 TS 侧 `chunkBuffer` 的默认值保持一致。
pub const DEFAULT_BLOCK_LEN: usize = 4096;

/// 按固定块大小切分缓冲区，产出 `(块, 该块在文件中的偏移量)`。
///
/// 对应 TS 侧 `src/decrypt-worker/util/buffer.ts` 的 `chunkBuffer`。
/// 返回的块是可变借用，供解密器原地解密。
pub fn chunk_buffer<'a>(
    buffer: &'a mut [u8],
    block_len: usize,
) -> impl Iterator<Item = (&'a mut [u8], usize)> + 'a {
    assert!(block_len > 0, "block_len 必须大于 0");
    buffer
        .chunks_mut(block_len)
        .enumerate()
        .map(move |(index, chunk)| (chunk, index * block_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_blocks_with_correct_offsets() {
        let mut data = vec![0u8; 10];
        let blocks: Vec<usize> = chunk_buffer(&mut data, 4).map(|(_, off)| off).collect();
        assert_eq!(blocks, vec![0, 4, 8]);

        let mut data = vec![0u8; 10];
        let sizes: Vec<usize> = chunk_buffer(&mut data, 4).map(|(b, _)| b.len()).collect();
        assert_eq!(sizes, vec![4, 4, 2]);
    }

    #[test]
    fn empty_buffer_yields_nothing() {
        let mut data: Vec<u8> = Vec::new();
        assert_eq!(chunk_buffer(&mut data, 4).count(), 0);
    }

    #[test]
    fn blocks_are_mutable_in_place() {
        let mut data = vec![1u8, 2, 3, 4, 5];
        for (block, _) in chunk_buffer(&mut data, 2) {
            block.fill(9);
        }
        assert_eq!(data, vec![9, 9, 9, 9, 9]);
    }
}
