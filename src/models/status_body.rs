use crate::models::status_dto::Status;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StatusBody {
    pub id: i32,
    pub status: Status,
    pub message: String,
}

impl StatusBody {
    pub fn new() -> Self {
        StatusBody {
            id: 0,
            status: Status::Unknown,
            message: String::new(),
        }
    }
}
