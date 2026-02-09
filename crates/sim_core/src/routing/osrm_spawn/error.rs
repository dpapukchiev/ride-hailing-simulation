/// Errors encountered while snapping a candidate trace.
#[derive(Debug)]
pub enum OsrmSpawnError {
    Http(reqwest::Error),
    Json(reqwest::Error),
    Api(String),
    NoMatch,
}

impl From<reqwest::Error> for OsrmSpawnError {
    fn from(err: reqwest::Error) -> Self {
        OsrmSpawnError::Http(err)
    }
}
