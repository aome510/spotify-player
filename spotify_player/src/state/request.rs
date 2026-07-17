use std::{collections::HashMap, time::Duration};

const SUCCESS_FEEDBACK_DURATION: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RequestKey {
    BrowseCategories,
    BrowseCategory(String),
    Context(String),
    CreatePlaylist,
    CurrentUser,
    Devices,
    Lyrics(String),
    Queue,
    Search(String),
    UserFollowedArtists,
    UserPlaylists,
    UserSavedAlbums,
    UserSavedShows,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestStatus {
    Loading,
    Succeeded,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestFeedbackKind {
    Pending,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct RequestFeedback {
    pub kind: RequestFeedbackKind,
    pub message: String,
    updated_at: std::time::Instant,
}

#[derive(Clone, Debug)]
pub struct RequestOperation {
    pending: String,
    success: String,
    failure: String,
}

impl RequestOperation {
    pub fn new(
        pending: impl Into<String>,
        success: impl Into<String>,
        failure: impl Into<String>,
    ) -> Self {
        Self {
            pending: pending.into(),
            success: success.into(),
            failure: failure.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RequestMetadata {
    pub key: Option<RequestKey>,
    pub operation: Option<RequestOperation>,
}

impl RequestMetadata {
    pub fn tracked(key: RequestKey) -> Self {
        Self {
            key: Some(key),
            operation: None,
        }
    }

    pub fn operation(operation: RequestOperation) -> Self {
        Self {
            key: None,
            operation: Some(operation),
        }
    }

    pub fn tracked_operation(key: RequestKey, operation: RequestOperation) -> Self {
        Self {
            key: Some(key),
            operation: Some(operation),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RequestToken {
    id: u64,
    key: Option<RequestKey>,
    operation: Option<RequestOperation>,
}

#[derive(Clone, Debug)]
struct TrackedStatus {
    request_id: u64,
    status: RequestStatus,
}

#[derive(Clone, Debug)]
struct TrackedFeedback {
    request_id: u64,
    feedback: RequestFeedback,
}

#[derive(Debug, Default)]
pub struct RequestTracker {
    next_id: u64,
    statuses: HashMap<RequestKey, TrackedStatus>,
    feedback: Option<TrackedFeedback>,
}

impl RequestTracker {
    pub fn begin(&mut self, metadata: RequestMetadata) -> RequestToken {
        self.next_id = self.next_id.wrapping_add(1);
        let token = RequestToken {
            id: self.next_id,
            key: metadata.key,
            operation: metadata.operation,
        };

        if let Some(key) = token.key.clone() {
            self.statuses.insert(
                key,
                TrackedStatus {
                    request_id: token.id,
                    status: RequestStatus::Loading,
                },
            );
        }
        if let Some(operation) = token.operation.as_ref() {
            self.feedback = Some(TrackedFeedback {
                request_id: token.id,
                feedback: RequestFeedback {
                    kind: RequestFeedbackKind::Pending,
                    message: operation.pending.clone(),
                    updated_at: std::time::Instant::now(),
                },
            });
        }

        token
    }

    pub fn succeed(&mut self, token: &RequestToken) {
        self.update_status(token, RequestStatus::Succeeded);
        if let Some(operation) = token.operation.as_ref() {
            self.update_feedback(
                token,
                RequestFeedbackKind::Success,
                operation.success.clone(),
            );
        }
    }

    pub fn fail(&mut self, token: &RequestToken, error: &str) {
        self.update_status(token, RequestStatus::Failed(error.to_owned()));
        if let Some(operation) = token.operation.as_ref() {
            self.update_feedback(
                token,
                RequestFeedbackKind::Error,
                format!("{}: {error}. See Logs for details.", operation.failure),
            );
        }
    }

    pub fn status(&self, key: &RequestKey) -> Option<&RequestStatus> {
        self.statuses.get(key).map(|status| &status.status)
    }

    pub fn is_loading(&self, key: &RequestKey) -> bool {
        self.status(key) == Some(&RequestStatus::Loading)
    }

    pub fn clear_status(&mut self, key: &RequestKey) {
        self.statuses.remove(key);
    }

    pub fn visible_feedback(&self) -> Option<RequestFeedback> {
        let tracked = self.feedback.as_ref()?;
        if tracked.feedback.kind == RequestFeedbackKind::Success
            && tracked.feedback.updated_at.elapsed() > SUCCESS_FEEDBACK_DURATION
        {
            return None;
        }
        Some(tracked.feedback.clone())
    }

    fn update_status(&mut self, token: &RequestToken, status: RequestStatus) {
        if let Some(key) = token.key.as_ref() {
            let Some(tracked) = self.statuses.get_mut(key) else {
                return;
            };
            if tracked.request_id == token.id {
                tracked.status = status;
            }
        }
    }

    fn update_feedback(
        &mut self,
        token: &RequestToken,
        kind: RequestFeedbackKind,
        message: String,
    ) {
        let Some(tracked) = self.feedback.as_mut() else {
            return;
        };
        if tracked.request_id == token.id {
            tracked.feedback = RequestFeedback {
                kind,
                message,
                updated_at: std::time::Instant::now(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RequestFeedbackKind, RequestKey, RequestMetadata, RequestOperation, RequestStatus,
        RequestTracker,
    };

    #[test]
    fn older_completion_cannot_replace_newer_status() {
        let mut tracker = RequestTracker::default();
        let older = tracker.begin(RequestMetadata::tracked(RequestKey::Queue));
        let newer = tracker.begin(RequestMetadata::tracked(RequestKey::Queue));

        tracker.fail(&older, "old failure");
        assert_eq!(
            tracker.status(&RequestKey::Queue),
            Some(&RequestStatus::Loading)
        );

        tracker.succeed(&newer);
        assert_eq!(
            tracker.status(&RequestKey::Queue),
            Some(&RequestStatus::Succeeded)
        );
    }

    #[test]
    fn operation_feedback_transitions_from_pending_to_error() {
        let mut tracker = RequestTracker::default();
        let token = tracker.begin(RequestMetadata::operation(RequestOperation::new(
            "Saving playlist",
            "Playlist saved",
            "Could not save playlist",
        )));

        assert_eq!(
            tracker.visible_feedback().unwrap().kind,
            RequestFeedbackKind::Pending
        );

        tracker.fail(&token, "network unavailable");
        let feedback = tracker.visible_feedback().unwrap();
        assert_eq!(feedback.kind, RequestFeedbackKind::Error);
        assert!(feedback.message.contains("network unavailable"));
        assert!(feedback.message.contains("See Logs"));
    }
}
