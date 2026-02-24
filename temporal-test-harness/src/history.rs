use temporalio_common::protos::{
    coresdk::IntoPayloadsExt,
    temporal::api::{
        common::v1::{ActivityType, Payload, Payloads},
        enums::v1::EventType,
        failure::v1::Failure,
        history::v1::{
            ActivityTaskCompletedEventAttributes, ActivityTaskFailedEventAttributes,
            ActivityTaskScheduledEventAttributes, ActivityTaskStartedEventAttributes,
            history_event::Attributes,
        },
    },
};
use temporalio_sdk_core::replay::TestHistoryBuilder;

/// A single mocked activity result.
pub enum ActivityMock {
    /// The activity completed successfully with the given JSON-serializable payload.
    Success(Payload),
    /// The activity failed with the given error message.
    Failure(String),
}

/// Build a synthetic workflow history from activity mocks.
///
/// Returns `(history_builder, has_failure)` where `has_failure` is true if any
/// activity was mocked as a failure (stops adding activities after the first failure).
pub fn build_history(
    workflow_type: &str,
    input_payloads: Payloads,
    activity_mocks: &[(String, ActivityMock)],
) -> (TestHistoryBuilder, bool) {
    let mut t = TestHistoryBuilder::default();
    t.add_by_type(EventType::WorkflowExecutionStarted);
    t.set_wf_type(workflow_type);
    t.set_wf_input(input_payloads);

    let mut has_failure = false;
    for (i, (activity_name, mock)) in activity_mocks.iter().enumerate() {
        let activity_id = (i + 1).to_string();

        // Add a full WFT before each activity (workflow task scheduled + started + completed)
        t.add_full_wf_task();

        let scheduled_event_id = t.add(ActivityTaskScheduledEventAttributes {
            activity_id: activity_id.clone(),
            activity_type: Some(ActivityType {
                name: activity_name.clone(),
            }),
            ..Default::default()
        });
        let started_event_id = t.add(Attributes::ActivityTaskStartedEventAttributes(
            ActivityTaskStartedEventAttributes {
                scheduled_event_id,
                ..Default::default()
            },
        ));

        match mock {
            ActivityMock::Success(payload) => {
                t.add(ActivityTaskCompletedEventAttributes {
                    scheduled_event_id,
                    started_event_id,
                    result: vec![payload.clone()].into_payloads(),
                    ..Default::default()
                });
            }
            ActivityMock::Failure(message) => {
                t.add(Attributes::ActivityTaskFailedEventAttributes(
                    ActivityTaskFailedEventAttributes {
                        scheduled_event_id,
                        started_event_id,
                        failure: Some(Failure {
                            message: message.clone(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ));
                has_failure = true;
                break; // Stop after first failure
            }
        }
    }

    // Final WFT scheduled + started (for the workflow to process the last result or to complete)
    t.add_workflow_task_scheduled_and_started();

    (t, has_failure)
}
