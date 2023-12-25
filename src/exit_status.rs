#[derive(Debug)]
pub enum ExitStatus<Error> {
    Success,
    Error(Error),
    FatalError(Error),
}
