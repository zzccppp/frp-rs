use bytes::{Buf, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ForwardPackage {
    Transport {
        uuid: Uuid,
        data: BytesMut,
    }, 
    HeartBeat{
        state: u32
    }
}

pub fn process_forward_bytes(buf: &mut BytesMut) -> Option<ForwardPackage> {
    if !buf.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE]) {
        drop_bytes_until_magic_number(buf);
        return None;
    } else {
        if buf.len() > 8 {
            let size = buf.get(4..8).unwrap();
            let size = size.clone().get_u32();
            if buf.len() >= (size + 8) as usize {
                let mut ret = buf.split_to(size as usize + 8);
                ret.advance(8);
                if let Ok(packet) = bincode::deserialize::<ForwardPackage>(ret.chunk()) {
                    return Some(packet);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
}

fn drop_bytes_until_magic_number(buf: &mut BytesMut) {
    for i in 0..buf.len() {
        if buf[i].eq(&0xCAu8) {
            buf.advance(i);
        }
    }
}