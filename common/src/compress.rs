use anyhow::Result;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::io::prelude::*;

const MIN_LENGTH: usize = 100;
const COMP_LEVEL: u32 = 6;

pub fn compress_if_needed(uncompressed: &mut Vec<u8>) -> Result<Option<Vec<u8>>> {
    if uncompressed.len() < MIN_LENGTH {
        return Ok(None);
    }
    let uncompressed = uncompressed as &[u8];
    let before = uncompressed.len();
    let mut e = ZlibEncoder::new(Vec::new(), Compression::new(COMP_LEVEL));
    e.write_all(uncompressed)?;
    let compressed = e.finish()?;

    let after = compressed.len();
    if after >= before {
        return Ok(None);
    }
    /* info!(
        "copress: {} => {}     {}",
        before,
        after,
        (after as f64) / (before as f64)
    ); */
    Ok(Some(compressed))
}
pub fn decompress(compressed: &mut Vec<u8>) -> Result<Vec<u8>> {
    let mut decompressed = Vec::new();
    let compressed = compressed as &[u8];
    // let before = compressed.len();
    let mut d = ZlibDecoder::new(compressed);
    d.read_to_end(&mut decompressed)?;
    // let after = decompressed.len();
    /* info!(
        "decopress: {} => {}     {}",
        before,
        after,
        (after as f64) / (before as f64)
    ); */
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_brotli() {
        let mut src = [1u8].repeat(MIN_LENGTH - 1);
        let res = compress_if_needed(&mut src);
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());

        let mut src = [1u8].repeat(MIN_LENGTH);
        let res = compress_if_needed(&mut src);
        assert!(res.is_ok());
        let mut res = res.unwrap().unwrap();
        assert_ne!(src, res);

        let res = decompress(&mut res);
        assert!(res.is_ok(), "{:?}", res);
        let res = res.unwrap();
        assert_eq!(src, res);
    }
}
