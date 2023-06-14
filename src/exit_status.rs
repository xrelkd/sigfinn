#[derive(Debug)]
pub enum ExitStatus<Error> {
    Success,
    Failure(Error),
}
