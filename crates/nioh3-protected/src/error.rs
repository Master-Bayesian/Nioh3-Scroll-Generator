//! The protected wire error shape and its one code.
//!
//! `protected_worker.py` answers every failure as
//! `{'code': getattr(error, 'code', 'OPERATION_REJECTED'), 'message': str(error)}`.
//! Only the shipped `CoreError` family carries a `.code`; every `ValueError`,
//! `RuntimeError` and adapter exception is reported as `OPERATION_REJECTED` with
//! its text, including the protocol tokens, which live *inside* the message
//! (`INVALID_REQUEST: ...`, `HANDSHAKE_REQUIRED`, `ROLE_MISMATCH`,
//! `INVALID_RESULT: ...`). This type keeps that exact behaviour so the Rust host
//! cannot invent a finer wire code than the shipped one.

use std::fmt;

/// Whether an error carries the shipped `CoreError` code or the default.
///
/// `code == None` means the raised Python exception had no `.code` attribute,
/// so the two shipped handlers pick their own default: the inline handler
/// answers `OPERATION_REJECTED`, while a failed protected job answers
/// `OPERATION_FAILED` (`protected_jobs.py`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError {
    pub code: Option<&'static str>,
    pub message: String,
}

impl HostError {
    /// A `ValueError`/`RuntimeError`-shaped failure with no shipped code.
    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
        }
    }

    /// A shipped `CoreError`-coded failure (`core_services.CoreErrorCode`).
    pub fn coded(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: Some(code),
            message: message.into(),
        }
    }

    /// The code the inline handler answers with.
    pub fn inline_code(&self) -> &'static str {
        self.code.unwrap_or("OPERATION_REJECTED")
    }

    /// The code a failed protected job answers with.
    pub fn job_code(&self) -> &'static str {
        self.code.unwrap_or("OPERATION_FAILED")
    }

    /// The contract-validation refusal, verbatim from `protected_worker.py`.
    pub fn invalid_request() -> Self {
        Self::rejected("INVALID_REQUEST: protected contract validation failed")
    }

    /// A result the response contract refuses, verbatim from `protected_worker.py`.
    pub fn invalid_result() -> Self {
        Self::rejected(
            "INVALID_RESULT: operation output does not match the protected contract; \
             query the operation receipt before retrying a write",
        )
    }

    pub fn handshake_required() -> Self {
        Self::rejected("HANDSHAKE_REQUIRED")
    }

    pub fn role_mismatch() -> Self {
        Self::rejected("ROLE_MISMATCH")
    }

    /// A refused save adapter error, reported with its own message.
    pub fn from_save(error: nioh3_save::SaveReadError) -> Self {
        Self::rejected(error.to_string())
    }

    /// A refused runtime adapter error, reported with its own message.
    ///
    /// The crate-internal machine code is deliberately *not* promoted to the
    /// wire: the shipped host would report `OPERATION_REJECTED` here.
    pub fn from_runtime(error: nioh3_runtime::RuntimeError) -> Self {
        Self::rejected(error.to_string())
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}
