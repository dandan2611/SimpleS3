use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use quick_xml::Writer;
use quick_xml::events::BytesText;
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum S3Error {
    #[error("The specified bucket does not exist")]
    NoSuchBucket,
    #[error("The specified key does not exist")]
    NoSuchKey,
    #[error("The specified upload does not exist")]
    NoSuchUpload,
    #[error("The requested bucket name already exists")]
    BucketAlreadyExists,
    #[error("The bucket you tried to delete is not empty")]
    BucketNotEmpty,
    #[error("Access Denied")]
    AccessDenied,
    #[error("The request signature we calculated does not match the signature you provided")]
    SignatureDoesNotMatch,
    #[error("Invalid part")]
    InvalidPart,
    #[error("Invalid part order")]
    InvalidPartOrder,
    #[error("The lifecycle configuration does not exist")]
    NoSuchLifecycleConfiguration,
    #[error("The bucket policy does not exist")]
    NoSuchBucketPolicy,
    #[error("The CORS configuration does not exist for this bucket")]
    NoSuchCORSConfiguration,
    #[error("Invalid argument")]
    InvalidArgument(String),
    #[error("Internal server error")]
    InternalError(String),
}

impl S3Error {
    pub fn code(&self) -> &str {
        match self {
            S3Error::NoSuchBucket => "NoSuchBucket",
            S3Error::NoSuchKey => "NoSuchKey",
            S3Error::NoSuchUpload => "NoSuchUpload",
            S3Error::BucketAlreadyExists => "BucketAlreadyOwnedByYou",
            S3Error::BucketNotEmpty => "BucketNotEmpty",
            S3Error::AccessDenied => "AccessDenied",
            S3Error::SignatureDoesNotMatch => "SignatureDoesNotMatch",
            S3Error::InvalidPart => "InvalidPart",
            S3Error::InvalidPartOrder => "InvalidPartOrder",
            S3Error::NoSuchLifecycleConfiguration => "NoSuchLifecycleConfiguration",
            S3Error::NoSuchBucketPolicy => "NoSuchBucketPolicy",
            S3Error::NoSuchCORSConfiguration => "NoSuchCORSConfiguration",
            S3Error::InvalidArgument(_) => "InvalidArgument",
            S3Error::InternalError(_) => "InternalError",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            S3Error::NoSuchBucket
            | S3Error::NoSuchKey
            | S3Error::NoSuchUpload
            | S3Error::NoSuchLifecycleConfiguration
            | S3Error::NoSuchBucketPolicy
            | S3Error::NoSuchCORSConfiguration => StatusCode::NOT_FOUND,
            S3Error::BucketAlreadyExists => StatusCode::CONFLICT,
            S3Error::BucketNotEmpty => StatusCode::CONFLICT,
            S3Error::AccessDenied | S3Error::SignatureDoesNotMatch => StatusCode::FORBIDDEN,
            S3Error::InvalidPart | S3Error::InvalidPartOrder | S3Error::InvalidArgument(_) => {
                StatusCode::BAD_REQUEST
            }
            S3Error::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn to_xml(&self) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer
            .create_element("Error")
            .write_inner_content(|w| {
                w.create_element("Code")
                    .write_text_content(BytesText::new(self.code()))?;
                w.create_element("Message")
                    .write_text_content(BytesText::new(&self.to_string()))?;
                Ok(())
            })
            .unwrap();
        let bytes = writer.into_inner().into_inner();
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}", String::from_utf8(bytes).unwrap())
    }
}

impl IntoResponse for S3Error {
    fn into_response(self) -> Response {
        let status = self.status_code();
        // Log internal errors server-side but don't leak details to clients
        if let S3Error::InternalError(ref detail) = self {
            tracing::error!(detail = %detail, "Internal server error");
        }
        let body = self.to_xml();
        (status, [("content-type", "application/xml")], body).into_response()
    }
}
