use std::io;

use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    Hello {
        peer_id: u64,
        chain_id: u64,
        best_height: u64,
        best_hash: [u8; 32],
    },
    NewBlock {
        height: u64,
        block_data: Vec<u8>,
    },
    NewTransaction {
        tx_data: Vec<u8>,
    },
    ConsensusMsg {
        msg_data: Vec<u8>,
    },
    GetBlocks {
        from_height: u64,
        count: u32,
    },
    Blocks {
        blocks: Vec<Vec<u8>>,
    },
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MessageCodec;

impl Decoder for MessageCodec {
    type Item = WireMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let mut len_buf = &src[..4];
        let len = len_buf.get_u32() as usize;
        if len > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("message too large: {len} bytes"),
            ));
        }

        if src.len() < 4 + len {
            return Ok(None);
        }

        src.advance(4);
        let payload = src.split_to(len);
        let msg = serde_json::from_slice::<WireMessage>(&payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid message json: {err}"),
            )
        })?;

        Ok(Some(msg))
    }
}

impl Encoder<WireMessage> for MessageCodec {
    type Error = io::Error;

    fn encode(&mut self, item: WireMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let payload = serde_json::to_vec(&item).map_err(|err| {
            io::Error::new(io::ErrorKind::InvalidData, format!("encode failed: {err}"))
        })?;
        let len = payload.len();

        if len > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("message too large: {len} bytes"),
            ));
        }

        dst.reserve(4 + len);
        dst.put_u32(len as u32);
        dst.extend_from_slice(&payload);
        Ok(())
    }
}
