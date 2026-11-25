//! The per-job attempt budget that bounds a retry decision.

use std::num::NonZeroU32;

/// How many deliveries one job may still receive.
///
/// The queue policy holds the operator's ceiling, but a job also carries a
/// limit of its own: the queue's stored task context records the
/// `max_attempts` the entry was enqueued with, and that value decides when the
/// entry stops being fetched. A retry decision therefore needs both numbers.
/// Keeping the job's limit as a value the domain understands lets
/// [`JobRetryPolicy::decide`](super::JobRetryPolicy::decide) answer the whole
/// question, instead of leaving the adapter to apply a second comparison after
/// the policy has already spoken.
///
/// The count is exclusive, matching the policy's own attempt limit: an attempt
/// below the budget may be retried, and an attempt at or above it may not.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU32;
///
/// use backend::domain::jobs::retry::JobAttemptBudget;
///
/// let budget = JobAttemptBudget::from_max_attempts(3);
/// assert!(budget.permits(NonZeroU32::new(2).expect("positive attempt")));
/// assert!(!budget.permits(NonZeroU32::new(3).expect("positive attempt")));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobAttemptBudget(NonZeroU32);

impl JobAttemptBudget {
    /// A budget that never binds before the policy's own attempt limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::JobAttemptBudget;
    ///
    /// let budget = JobAttemptBudget::unlimited();
    /// assert!(budget.permits(NonZeroU32::new(u32::MAX - 1).expect("positive attempt")));
    /// ```
    pub const fn unlimited() -> Self {
        Self(NonZeroU32::MAX)
    }

    /// Construct a budget from a positive attempt count.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::JobAttemptBudget;
    ///
    /// let budget = JobAttemptBudget::new(NonZeroU32::new(2).expect("positive attempt count"));
    /// assert_eq!(budget.max_attempts().get(), 2);
    /// ```
    pub const fn new(max_attempts: NonZeroU32) -> Self {
        Self(max_attempts)
    }

    /// Interpret a stored or configured attempt cap.
    ///
    /// A zero cap permits no attempt at all, because an entry enqueued with no
    /// retry allowance is terminal on its first failure. Zero is held as the
    /// smallest positive count, which an always-positive attempt can never
    /// satisfy, so the zero case needs no separate branch at the decision site.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::JobAttemptBudget;
    ///
    /// let attempt = NonZeroU32::new(1).expect("positive attempt");
    /// assert!(!JobAttemptBudget::from_max_attempts(0).permits(attempt));
    /// assert!(!JobAttemptBudget::from_max_attempts(1).permits(attempt));
    /// ```
    pub const fn from_max_attempts(max_attempts: u32) -> Self {
        match NonZeroU32::new(max_attempts) {
            Some(budget) => Self(budget),
            None => Self(NonZeroU32::MIN),
        }
    }

    /// The exclusive attempt count this budget allows.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::jobs::retry::JobAttemptBudget;
    ///
    /// assert_eq!(JobAttemptBudget::from_max_attempts(4).max_attempts().get(), 4);
    /// ```
    pub const fn max_attempts(self) -> NonZeroU32 {
        self.0
    }

    /// Report whether `attempt` falls inside this budget.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::JobAttemptBudget;
    ///
    /// let budget = JobAttemptBudget::from_max_attempts(2);
    /// assert!(budget.permits(NonZeroU32::new(1).expect("positive attempt")));
    /// assert!(!budget.permits(NonZeroU32::new(2).expect("positive attempt")));
    /// ```
    pub const fn permits(self, attempt: NonZeroU32) -> bool {
        attempt.get() < self.0.get()
    }
}

/// An attempt paired with the job budget it has to satisfy.
///
/// Both attempt ceilings a retry decision weighs — the policy's and the job's —
/// act on the same attempt count, so holding the pair together means a call
/// site cannot hand the policy an attempt and a budget that describe different
/// deliveries. The policy's own ceiling stays with the policy, because it is
/// configuration rather than a property of this delivery.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU32;
///
/// use backend::domain::jobs::retry::{BoundedAttempt, JobAttemptBudget};
///
/// let attempt = BoundedAttempt::new(
///     NonZeroU32::new(2).expect("positive attempt"),
///     JobAttemptBudget::from_max_attempts(3),
/// );
/// assert!(attempt.is_permitted());
/// assert_eq!(attempt.attempt().get(), 2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundedAttempt {
    attempt: NonZeroU32,
    budget: JobAttemptBudget,
}

impl BoundedAttempt {
    /// Pair the attempt that just failed with its job's budget.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::{BoundedAttempt, JobAttemptBudget};
    ///
    /// let attempt = BoundedAttempt::new(
    ///     NonZeroU32::new(1).expect("positive attempt"),
    ///     JobAttemptBudget::unlimited(),
    /// );
    /// assert_eq!(attempt.budget(), JobAttemptBudget::unlimited());
    /// ```
    pub const fn new(attempt: NonZeroU32, budget: JobAttemptBudget) -> Self {
        Self { attempt, budget }
    }

    /// The attempt that just failed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::{BoundedAttempt, JobAttemptBudget};
    ///
    /// let attempt = BoundedAttempt::new(
    ///     NonZeroU32::new(4).expect("positive attempt"),
    ///     JobAttemptBudget::unlimited(),
    /// );
    /// assert_eq!(attempt.attempt().get(), 4);
    /// ```
    pub const fn attempt(self) -> NonZeroU32 {
        self.attempt
    }

    /// The budget that attempt must satisfy.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::{BoundedAttempt, JobAttemptBudget};
    ///
    /// let attempt = BoundedAttempt::new(
    ///     NonZeroU32::new(1).expect("positive attempt"),
    ///     JobAttemptBudget::from_max_attempts(2),
    /// );
    /// assert_eq!(attempt.budget().max_attempts().get(), 2);
    /// ```
    pub const fn budget(self) -> JobAttemptBudget {
        self.budget
    }

    /// Report whether this attempt falls inside its job's budget.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    ///
    /// use backend::domain::jobs::retry::{BoundedAttempt, JobAttemptBudget};
    ///
    /// let attempt = NonZeroU32::new(2).expect("positive attempt");
    /// let roomy = BoundedAttempt::new(attempt, JobAttemptBudget::from_max_attempts(3));
    /// let spent = BoundedAttempt::new(attempt, JobAttemptBudget::from_max_attempts(2));
    /// assert!(roomy.is_permitted());
    /// assert!(!spent.is_permitted());
    /// ```
    pub const fn is_permitted(self) -> bool {
        self.budget.permits(self.attempt)
    }
}
