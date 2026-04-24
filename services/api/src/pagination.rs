use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginationMeta {
    pub cursor: Option<String>,
    pub limit: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationMeta,
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self { cursor: None, limit: None }
    }
}

impl PaginationQuery {
    pub fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(20)
            .clamp(1, 100)
    }

    pub fn cursor(&self) -> Option<String> {
        self.cursor.clone()
    }
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, cursor: Option<String>, limit: i64, has_more: bool) -> Self {
        Self {
            data,
            pagination: PaginationMeta {
                cursor,
                limit,
                has_more,
            },
        }
    }
}
