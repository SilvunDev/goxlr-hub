use crate::TransportError;

/// Why a read gave no answer.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The device has not finished the command yet.
    NotReady(String),
    Failed(String),
}

/// Asks for the answer until the device has it.
pub(crate) fn read_when_ready(
    attempts: u32,
    mut read: impl FnMut() -> Result<Vec<u8>, ReadError>,
    mut wait: impl FnMut(),
) -> Result<Vec<u8>, TransportError> {
    let mut last = String::from("never asked");
    for attempt in 0..attempts {
        if attempt > 0 {
            wait();
        }
        match read() {
            Ok(answer) => return Ok(answer),
            Err(ReadError::NotReady(reason)) => last = reason,
            Err(ReadError::Failed(reason)) => return Err(TransportError::Io(reason)),
        }
    }
    Err(TransportError::NoAnswer(last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_an_answer_that_is_not_ready_yet() {
        let mut reads = 0;
        let mut waits = 0;
        let answer = read_when_ready(
            20,
            || {
                reads += 1;
                if reads < 3 {
                    Err(ReadError::NotReady("stalled".into()))
                } else {
                    Ok(vec![7])
                }
            },
            || waits += 1,
        );
        assert_eq!(answer, Ok(vec![7]));
        assert_eq!((reads, waits), (3, 2));
    }

    #[test]
    fn gives_up_on_a_device_that_never_answers() {
        let mut reads = 0;
        let answer = read_when_ready(
            20,
            || {
                reads += 1;
                Err(ReadError::NotReady("stalled".into()))
            },
            || {},
        );
        assert_eq!(answer, Err(TransportError::NoAnswer("stalled".into())));
        assert_eq!(reads, 20);
    }

    #[test]
    fn stops_at_once_on_a_real_failure() {
        let mut reads = 0;
        let answer = read_when_ready(
            20,
            || {
                reads += 1;
                Err(ReadError::Failed("unplugged".into()))
            },
            || {},
        );
        assert_eq!(answer, Err(TransportError::Io("unplugged".into())));
        assert_eq!(reads, 1);
    }
}
