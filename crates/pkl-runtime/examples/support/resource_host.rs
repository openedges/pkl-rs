//! File ResourceReader for the development original-module adapter. The engine
//! owns URI resolution, cache identity, absence handling, and Resource values.
use pkl_runtime::{
    host::ReadFailure,
    resources::{ReadResult, Resource},
    vm::ReadRequest,
};

pub fn complete(request: &ReadRequest) -> Option<Result<ReadResult, ReadFailure>> {
    let uri = request.resolved_uri().ok()?;
    // Other schemes use the shared configured policy; the adapter does not
    // convert missing capabilities into successful absence.
    let path = super::module_host::file_path(&uri).ok()?;
    Some(match std::fs::read(path) {
        Ok(bytes) => Ok(ReadResult::Resource(Resource {
            uri,
            bytes: bytes.into(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ReadResult::Missing),
        Err(error) => Err(ReadFailure::Io(error.to_string().into())),
    })
}
