//! Identifier newtypes shared by the canary fixtures.
//!
//! Every identifier a canary arranges is a string in PostgreSQL: the queue a
//! row belongs to, the worker name that locks it, the Apalis job id, and the
//! claim token a lock stamps on the delivery. Passing them as `&str` lets a
//! caller swap two of them and still compile, so each gets its own type here.
//! The wrappers also keep the fixture signatures from being string-heavy,
//! which the code-health gate charges against a changed file.

/// The queue a job row belongs to.
///
/// Apalis matches a worker row's `worker_type` against the queue when it
/// recovers orphaned rows, so the queue is also the recovery scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QueueName<'a>(pub(crate) &'a str);

impl<'a> QueueName<'a> {
    /// Borrow the name as the `&str` a SQL binding takes.
    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for QueueName<'a> {
    fn from(name: &'a str) -> Self {
        Self(name)
    }
}

/// The identifier a worker registers under and stamps into `lock_by`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkerName<'a>(pub(crate) &'a str);

impl<'a> WorkerName<'a> {
    /// Borrow the name as the `&str` a SQL binding takes.
    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for WorkerName<'a> {
    fn from(name: &'a str) -> Self {
        Self(name)
    }
}

/// The Apalis job identifier, which keys every row a canary reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JobId<'a>(pub(crate) &'a str);

impl<'a> JobId<'a> {
    /// Borrow the identifier as the `&str` a SQL binding takes.
    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for JobId<'a> {
    fn from(id: &'a str) -> Self {
        Self(id)
    }
}

/// The token the lock step stamps on a row and carries in the delivery.
///
/// The acknowledgement compares it with the row's stored token, so it is the
/// only value that identifies a delivery generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClaimToken<'a>(pub(crate) &'a str);

impl<'a> ClaimToken<'a> {
    /// Borrow the token as the `&str` the context and SQL take.
    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for ClaimToken<'a> {
    fn from(token: &'a str) -> Self {
        Self(token)
    }
}
