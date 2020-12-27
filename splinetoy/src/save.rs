use serde::{Deserialize, Serialize};
use spline::SplineSpec;

use crate::edit_session::EditSession;
use crate::path::{Path, PointId};
use crate::tools::ToolId;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub paths: Vec<SplineSpec>,
    pub selection: Option<PointId>,
    pub tool: ToolId,
    pub select_only: bool,
}

const B64_HEADER_LEN: usize = 4;
const CURRENT_VERSION: u16 = 1;

#[derive(Debug, Clone)]
pub enum DataError {
    InvalidHeader,
    UnexpectedVersion(u16),
    #[cfg(target_arch = "wasm32")]
    MissingWindow,
    #[cfg(target_arch = "wasm32")]
    WebError(wasm_bindgen::JsValue),
}

type BoxErr = Box<dyn std::error::Error>;

impl SessionState {
    pub fn from_bytes(bytes: &[u8]) -> Result<SessionState, BoxErr> {
        let bytes = std::str::from_utf8(bytes)?.trim();
        if bytes.len() > B64_HEADER_LEN {
            let (header, body) = bytes.as_bytes().split_at(B64_HEADER_LEN);
            let version = decode_b64_header(&header).expect("header should always be well-formed");
            return match version {
                1 => SessionState::from_b64(&body),
                n => Err(DataError::UnexpectedVersion(n).into()),
            };
        }
        Err(DataError::InvalidHeader.into())
    }

    pub fn from_json(bytes: &[u8]) -> Result<SessionState, BoxErr> {
        let paths = serde_json::from_slice(bytes)?;
        Ok(SessionState {
            paths,
            ..Default::default()
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn init_from_current_url() -> Result<SessionState, BoxErr> {
        let window = web_sys::window().ok_or(DataError::MissingWindow)?;
        if let Ok(search_query) = window.location().search() {
            if !search_query.is_empty() {
                if let Ok(paths) = deserialize_just_paths_old(search_query) {
                    return Ok(SessionState {
                        paths,
                        ..Default::default()
                    });
                }
            }
        }

        let anchor = window.location().hash().map_err(DataError::WebError)?;
        if anchor.is_empty() {
            Ok(Default::default())
        } else {
            let bytes = anchor.trim_start_matches('#');
            Self::from_bytes(bytes.as_bytes())
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save_to_url(&self) {
        if let Err(e) = self.web_save_impl() {
            web_sys::console::log_1(&format!("save failed: {}", e).into());
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn web_save_impl(&self) -> Result<(), BoxErr> {
        web_sys::console::log_1(&format!("saving contents").into());
        if let Some(window) = web_sys::window() {
            let save_str = self.encode()?;
            web_sys::console::log_1(&format!("search: {:?}", window.location().search()).into());
            window.location().set_hash(&save_str).unwrap();
            web_sys::console::log_1(&format!("saved {} bytes to anchor", save_str.len()).into());
            Ok(())
        } else {
            Err(DataError::MissingWindow.into())
        }
    }

    pub fn into_edit_session(self) -> EditSession {
        //last path is the possibly active path
        let SessionState {
            paths, selection, ..
        } = self;
        let paths: Vec<_> = paths.into_iter().map(Path::from_spline).collect();
        EditSession::from_saved(paths, selection)
    }

    fn from_b64(bytes: &[u8]) -> Result<SessionState, Box<dyn std::error::Error>> {
        let bytes = base64::decode_config(bytes, base64::URL_SAFE)?;
        let mut r = flate2::read::ZlibDecoder::new(bytes.as_slice());
        bincode::deserialize_from(&mut r).map_err(Into::into)
    }

    pub fn encode(&self) -> Result<String, BoxErr> {
        use flate2::{write::ZlibEncoder, Compression};
        let mut buf = Vec::with_capacity(128);
        let header = encode_b64_header(CURRENT_VERSION);
        buf.extend_from_slice(&header);
        let b64_writer = base64::write::EncoderWriter::new(&mut buf, base64::URL_SAFE);
        let mut encoder = ZlibEncoder::new(b64_writer, Compression::default());
        bincode::serialize_into(&mut encoder, self).unwrap();
        encoder.finish().unwrap();
        String::from_utf8(buf).map_err(Into::into)
    }
}

#[cfg(target_arch = "wasm32")]
fn deserialize_just_paths_old(s: String) -> Result<Vec<SplineSpec>, BoxErr> {
    let b64 = s.trim_start_matches('?');
    let bytes = base64::decode(b64)?;
    // decode
    let mut r = flate2::read::ZlibDecoder::new(bytes.as_slice());
    bincode::deserialize_from(&mut r).map_err(Into::into)
}

fn encode_b64_header(version: u16) -> [u8; 4] {
    let mut out = [b'A'; 4];
    let bytes = version.to_be_bytes();
    base64::encode_config_slice(&bytes, base64::URL_SAFE, &mut out);
    out
}

fn decode_b64_header(header: &[u8]) -> Result<u16, base64::DecodeError> {
    assert_eq!(header.len(), B64_HEADER_LEN);
    let mut out = [b'0'; 2];
    base64::decode_config_slice(header, base64::URL_SAFE_NO_PAD, &mut out)?;
    Ok(u16::from_be_bytes(out))
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DataError::InvalidHeader => write!(f, "Invalid header"),
            DataError::UnexpectedVersion(v) => write!(f, "Unexpected version {}", v),
            #[cfg(target_arch = "wasm32")]
            DataError::MissingWindow => write!(f, "Missing window"),
            #[cfg(target_arch = "wasm32")]
            DataError::WebError(js_val) => write!(f, "Javascript error: '{:?}'", js_val),
        }
    }
}

impl std::error::Error for DataError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode() {
        for i in 0..=u16::MAX {
            assert_eq!(Ok(i), decode_b64_header(&encode_b64_header(i)))
        }
    }
}
