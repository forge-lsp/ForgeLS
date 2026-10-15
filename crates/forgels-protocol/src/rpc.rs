#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: String,
    pub method: String,
    pub params: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub id: String,
    pub result: Option<String>,
    pub error: Option<String>,
}
