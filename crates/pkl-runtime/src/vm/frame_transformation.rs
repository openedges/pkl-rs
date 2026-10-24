//! Frame callbacks suspend the same invocation as readers and log delegates.
//! No caller-owned callback executes while the VM or heap is borrowed.
use super::*;
use crate::logging::{CallbackOrigin, FrameTransformer, LogDelivery, LogEvent, LogFrame};

#[derive(Clone)]
pub(crate) struct PendingTransform {
    pub(super) request: FrameTransformRequest,
    continuation: Continuation,
}

#[derive(Clone)]
enum Continuation {
    Log(Arc<LogEvent>),
    Diagnostic {
        error: RuntimeError,
        rendering: Box<crate::diagnostic::PreparedDiagnostic>,
    },
}

impl Vm {
    pub(crate) fn set_frame_transformer(
        &mut self,
        transformer: FrameTransformer,
    ) -> Result<(), RuntimeError> {
        self.available()?;
        self.context.frame_transformer = transformer;
        Ok(())
    }

    fn next_frame_request_id(&mut self) -> Result<u64, RuntimeError> {
        let id = self.next_request;
        self.next_request = id
            .checked_add(1)
            .ok_or_else(|| invalid("Host request IDs exhausted"))?;
        Ok(id)
    }

    fn suspend_frame_transform(
        &mut self,
        frame: LogFrame,
        continuation: Continuation,
    ) -> Result<(), RuntimeError> {
        let id = self.next_frame_request_id()?;
        self.state = State::Waiting(PendingRequest::TransformFrame(Box::new(PendingTransform {
            request: FrameTransformRequest {
                id,
                frame: Arc::new(frame),
            },
            continuation,
        })));
        Ok(())
    }

    /// The caller has already appended the message to BufferedLogger. True
    /// means both transformation and delegate delivery completed synchronously.
    pub(super) fn begin_log_delivery(
        &mut self,
        event: Arc<LogEvent>,
    ) -> Result<bool, RuntimeError> {
        if self.context.frame_transformer == FrameTransformer::Host {
            self.suspend_frame_transform(event.frame.clone(), Continuation::Log(event))?;
            return Ok(false);
        }
        let mut event = (*event).clone();
        event.frame = self.context.frame_transformer.transform(event.frame);
        self.deliver_transformed_log(Arc::new(event))
    }

    fn deliver_transformed_log(&mut self, event: Arc<LogEvent>) -> Result<bool, RuntimeError> {
        if self.context.log_delivery == LogDelivery::Noop {
            return Ok(true);
        }
        let id = self.next_frame_request_id()?;
        self.state = State::Waiting(PendingRequest::Log(LogRequest { id, event }));
        Ok(false)
    }

    pub(super) fn finish_log_delivery(&mut self) -> Result<(), RuntimeError> {
        self.state = State::Running;
        if let Some(warning) = self
            .frames
            .last_mut()
            .and_then(|frame| frame.warning.take())
        {
            self.publish_method_call(warning)?;
        } else {
            self.frames
                .last_mut()
                .and_then(|frame| frame.deep_force.as_mut())
                .ok_or_else(|| invalid("log outside logger continuation"))?
                .acknowledge_log()?;
        }
        Ok(())
    }

    pub fn complete_frame_transform(
        &mut self,
        id: u64,
        completion: Result<LogFrame, LoggerFailure>,
    ) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(PendingRequest::TransformFrame(pending)) if pending.request.id == id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected Host completion",
            ));
        }
        let State::Waiting(PendingRequest::TransformFrame(pending)) =
            std::mem::replace(&mut self.state, State::Running)
        else {
            unreachable!("validated transform request")
        };
        match (pending.continuation, completion) {
            (Continuation::Log(event), Ok(frame)) => {
                let mut event = (*event).clone();
                event.frame = frame;
                if self.deliver_transformed_log(Arc::new(event))? {
                    self.finish_log_delivery()?;
                }
            }
            (Continuation::Log(_), Err(failure)) => {
                self.fail(RuntimeError::callback_failure(
                    CallbackOrigin::LogTransformer,
                    failure.message,
                ));
            }
            (
                Continuation::Diagnostic {
                    mut error,
                    mut rendering,
                },
                Ok(frame),
            ) => {
                if let Err(message) = crate::diagnostic::validate_transformed_frame(&frame) {
                    let mut error =
                        RuntimeError::callback_failure(CallbackOrigin::DiagnosticRenderer, message);
                    error.rendered_diagnostic = Some(error.message.clone());
                    self.state = State::Finished(Err(error));
                    return Ok(());
                }
                rendering.complete_frame(frame);
                if let Some(frame) = rendering.next_frame().cloned() {
                    self.suspend_frame_transform(
                        frame,
                        Continuation::Diagnostic { error, rendering },
                    )?;
                } else {
                    error.rendered_diagnostic = Some(rendering.render());
                    self.state = State::Finished(Err(error));
                }
            }
            (Continuation::Diagnostic { .. }, Err(failure)) => {
                // A failure of the exception formatter escapes it. Calling the
                // formatter again would repeat a failed callback indefinitely.
                let mut error = RuntimeError::callback_failure(
                    CallbackOrigin::DiagnosticTransformer,
                    failure.message,
                );
                error.rendered_diagnostic = Some(error.message.clone());
                self.state = State::Finished(Err(error));
            }
        }
        Ok(())
    }

    /// Cached language errors remain raw. Every escaping throw chooses the
    /// current evaluator policy and owns its own diagnostic continuation.
    pub(super) fn finish_error(&mut self, mut error: RuntimeError) {
        if error.frame_transformer != self.context.frame_transformer {
            error.frame_transformer = self.context.frame_transformer;
        }
        if error.rendered_diagnostic.is_some() {
            error.rendered_diagnostic = None;
        }
        if self.context.frame_transformer != FrameTransformer::Host
            || matches!(
                error.kind,
                ErrorKind::Cancelled | ErrorKind::Timeout | ErrorKind::Closed
            )
        {
            self.state = State::Finished(Err(error));
            return;
        }
        let source = self
            .public_primary_source()
            .or_else(|| self.context.primary_source.clone())
            .or_else(|| error.frames.iter().find_map(|frame| frame.source.clone()));
        let text = source.as_ref().map_or("", |source| source.text.as_ref());
        let uri = source
            .as_ref()
            .map_or("repl:text", |source| source.uri.as_ref());
        let module = source
            .as_ref()
            .map_or(self.program.module_name.as_ref(), |source| {
                source.module_name.as_ref()
            });
        let mut sources = crate::source::SourceMap::default();
        let source_id = sources.add(uri, text);
        let rendering = Box::new(crate::evaluation::prepare_runtime_error(
            sources.get(source_id).expect("diagnostic source"),
            uri,
            module,
            "",
            error.clone(),
            None,
            crate::evaluation::frontend::DiagnosticProfile::ApiIdentity,
        ));
        if let Some(frame) = rendering.next_frame().cloned() {
            if let Err(failure) =
                self.suspend_frame_transform(frame, Continuation::Diagnostic { error, rendering })
            {
                self.state = State::Finished(Err(failure));
            }
        } else {
            error.rendered_diagnostic = Some(rendering.render());
            self.state = State::Finished(Err(error));
        }
    }
}

#[cfg(test)]
mod tests;
