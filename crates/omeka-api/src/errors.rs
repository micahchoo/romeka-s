//! API error documents and status mapping.
//!
//! Status codes and the `{"errors": ...}` document mirror
//! `application/src/View/Strategy/ApiJsonStrategy.php` and
//! `application/src/View/Renderer/ApiJsonRenderer.php`. Two codes are port
//! decisions, not upstream ones: 405 for a routed path with a wrong method
//! (upstream answers 405 through the generic controller with a non-JSON body)
//! and 501 for a handler this wave does not build (upstream has no 501).

use std::collections::BTreeMap;

use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::messages;
use crate::routes::{Method, Operation, Resolution};
use crate::wire::common::JsonLdDate;

/// `ApiJsonRenderer` non-validation payload: `{"error": "<message>"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralError {
    pub error: String,
}

/// `ErrorStore::getErrors()`: `{"<key>": ["<message>", ...]}`.
pub type ValidationErrors = BTreeMap<String, Vec<String>>;

/// One entry of an `ErrorStore::mergeErrors($store, $key)` array.
///
/// `mergeErrors` with a key appends the whole nested error store under that
/// key, so the array mixes plain messages (`ItemAdapter`'s `Media must belong
/// to this item.`) with one nested store per failing sub-entity (a media whose
/// ingester refused the request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NestedError {
    /// A message added directly under the outer key.
    Message(String),
    /// A sub-entity's whole error store.
    Errors(ValidationErrors),
}

/// `ErrorStore::mergeErrors($store, $key)`: `{"<key>": [<entry>, ...]}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedErrors(pub BTreeMap<String, Vec<NestedError>>);

/// Wire error document: `{"errors": ...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDocument {
    pub errors: ErrorPayload,
}

impl ErrorDocument {
    pub fn general(message: impl Into<String>) -> Self {
        Self {
            errors: ErrorPayload::General(GeneralError {
                error: message.into(),
            }),
        }
    }

    pub fn validation(errors: ValidationErrors) -> Self {
        Self {
            errors: ErrorPayload::Validation(errors),
        }
    }

    /// `ErrorStore::mergeErrors($store, $key)`, one key deep.
    pub fn merged(key: impl Into<String>, errors: Vec<NestedError>) -> Self {
        let mut map = BTreeMap::new();
        map.insert(key.into(), errors);
        Self {
            errors: ErrorPayload::Merged(MergedErrors(map)),
        }
    }
}

/// The error shapes the renderer emits, kept distinguishable in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ErrorPayload {
    /// First, because an untagged read tries the variants in order and a stale-page document
    /// also has the `error` member a general one has.
    StalePage(StalePageError),
    General(GeneralError),
    Validation(ValidationErrors),
    Merged(MergedErrors),
}

/// The 409 body for a site page save whose `o:modified` is older than the stored page's (a Romeka
/// addition): the message, when the stored page was saved, and who saved it when that is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StalePageError {
    pub error: String,
    /// Required on a read, so a general error never parses as this one.
    #[serde(rename = "o:modified", deserialize_with = "Option::deserialize")]
    pub modified: Option<JsonLdDate>,
    #[serde(rename = "o:user")]
    pub user: Option<SavedBy>,
}

/// The user a [`StalePageError`] names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedBy {
    #[serde(rename = "o:id")]
    pub id: u64,
    #[serde(rename = "o:name")]
    pub name: String,
}

/// The typed 501 body for a handler wave 1 does not build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotImplemented {
    pub errors: NotImplementedErrors,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotImplementedErrors {
    pub error: String,
}

impl std::fmt::Display for NotImplemented {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.errors.error)
    }
}

impl NotImplemented {
    /// Mirrors the upstream `OperationNotImplementedException` sentence with
    /// the resource name in place of the PHP class name.
    pub fn adapter(resource: &str, operation: Operation) -> Self {
        Self {
            errors: NotImplementedErrors {
                error: messages::adapter_not_implemented(resource, operation.adapter_name()),
            },
        }
    }

    /// For routes that answer without an adapter (`/api`, `/api-context`).
    pub fn endpoint(endpoint: &str) -> Self {
        Self {
            errors: NotImplementedErrors {
                error: messages::endpoint_not_implemented(endpoint),
            },
        }
    }
}

/// An API failure with its HTTP status and wire document.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApiError {
    #[error("bad request: {message}")]
    BadRequest { message: String },
    #[error("permission denied: {message}")]
    PermissionDenied { message: String },
    #[error("not found: {message}")]
    NotFound { message: String },
    #[error("conflict: {message}")]
    Conflict { message: String },
    #[error("conflict: {}", .0.error)]
    StalePage(Box<StalePageError>),
    #[error("method not allowed")]
    MethodNotAllowed { allow: Vec<Method> },
    #[error("unsupported media type: {message}")]
    UnsupportedMediaType { message: String },
    #[error("validation failed")]
    Validation(ValidationErrors),
    #[error("validation failed")]
    MergedValidation {
        key: String,
        errors: Vec<NestedError>,
    },
    #[error("{0}")]
    NotImplemented(NotImplemented),
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    /// An [`ApiError`] with the message of any displayable failure.
    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }

    /// `ApiJsonStrategy::getStatusCodeForException`.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::PermissionDenied { .. } => StatusCode::FORBIDDEN,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::StalePage(_) => StatusCode::CONFLICT,
            Self::MethodNotAllowed { .. } => StatusCode::METHOD_NOT_ALLOWED,
            Self::UnsupportedMediaType { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::MergedValidation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The response body for this error.
    pub fn document(&self) -> ErrorDocument {
        match self {
            Self::BadRequest { message }
            | Self::PermissionDenied { message }
            | Self::NotFound { message }
            | Self::Conflict { message }
            | Self::UnsupportedMediaType { message }
            | Self::Internal { message } => ErrorDocument::general(message.clone()),
            Self::StalePage(stale) => ErrorDocument {
                errors: ErrorPayload::StalePage(stale.as_ref().clone()),
            },
            Self::MethodNotAllowed { .. } => ErrorDocument::general(messages::METHOD_NOT_ALLOWED),
            Self::Validation(errors) => ErrorDocument::validation(errors.clone()),
            Self::MergedValidation { key, errors } => {
                ErrorDocument::merged(key.clone(), errors.clone())
            }
            Self::NotImplemented(body) => ErrorDocument::general(body.errors.error.clone()),
        }
    }
}

impl From<Resolution> for ApiError {
    fn from(resolution: Resolution) -> Self {
        match resolution {
            Resolution::NotImplemented { target } => Self::NotImplemented(target.not_implemented()),
            Resolution::MethodNotAllowed { allow, .. } => Self::MethodNotAllowed {
                allow: allow.to_vec(),
            },
            Resolution::NotFound { message } => Self::not_found(message),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn status_codes_match_api_json_strategy() {
        assert_eq!(ApiError::bad_request("x").status(), StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::not_found("x").status(), StatusCode::NOT_FOUND);
        assert_eq!(
            ApiError::PermissionDenied {
                message: "x".into()
            }
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::UnsupportedMediaType {
                message: "x".into()
            }
            .status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            ApiError::Validation(ValidationErrors::new()).status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            ApiError::Internal {
                message: "x".into()
            }
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn general_document_is_the_errors_error_shape() {
        let doc = ApiError::not_found("Nothing here.").document();
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            r#"{"errors":{"error":"Nothing here."}}"#
        );
    }

    #[test]
    fn validation_document_is_the_error_store_shape() {
        let mut errors = ValidationErrors::new();
        errors.insert("o:title".to_string(), vec!["Value is required".to_string()]);
        assert_eq!(
            serde_json::to_string(&ApiError::Validation(errors).document()).unwrap(),
            r#"{"errors":{"o:title":["Value is required"]}}"#
        );
    }

    #[test]
    fn method_not_allowed_document_uses_the_laminas_text() {
        let err = ApiError::MethodNotAllowed {
            allow: vec![Method::Get],
        };
        assert_eq!(err.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            serde_json::to_string(&err.document()).unwrap(),
            r#"{"errors":{"error":"Method Not Allowed"}}"#
        );
    }
}
