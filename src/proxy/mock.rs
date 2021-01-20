use http::status::StatusCode;
use crate::proxy::Request;

#[derive(Debug, Clone)]
pub struct Response {
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub status: StatusCode,
}
impl Default for Response {
    fn default() -> Self {
        Self {
            body: Vec::new(),
            headers: Vec::new(),
            status: http::StatusCode::default(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Mock {
    pub path: String,
    pub method: String,
    pub response: Response,
}
impl Mock {
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            response: Response::default(),
        }
    }

    pub fn matches(&self, request: &Request) -> bool {
        &self.path == request.path.as_ref().unwrap() && &self.method == request.method.as_ref().unwrap()
    }
}
