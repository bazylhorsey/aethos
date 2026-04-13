use bytes::Bytes;

/// The response body accumulated during plug chain execution.
#[derive(Default, Debug, Clone)]
pub enum ResponseBody {
    #[default]
    Empty,
    Bytes(Bytes),
    Text(String),
}

impl ResponseBody {
    pub fn into_bytes(self) -> Bytes {
        match self {
            ResponseBody::Empty => Bytes::new(),
            ResponseBody::Bytes(b) => b,
            ResponseBody::Text(s) => Bytes::from(s),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, ResponseBody::Empty)
    }
}
