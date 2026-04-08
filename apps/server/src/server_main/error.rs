use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ErrorPayload {
    error: String,
    message: String,
    status: u16,
}

#[derive(Debug)]
pub(super) enum AppError {
    Unauthorized,
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Internal(anyhow!(value))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized".to_string(),
                "Authentication is required".to_string(),
            ),
            AppError::NotFound(message) => {
                (StatusCode::NOT_FOUND, "not_found".to_string(), message)
            }
            AppError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, "bad_request".to_string(), message)
            }
            AppError::Internal(error) => {
                error!("internal server error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_server_error".to_string(),
                    "Something went wrong".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorPayload {
                error,
                message,
                status: status.as_u16(),
            }),
        )
            .into_response()
    }
}

pub(super) type ApiResult<T> = Result<Json<T>, AppError>;
