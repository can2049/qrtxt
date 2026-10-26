//! Console events and the source abstraction used by the collector.

use std::io;

/// A character key event from the console input queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    /// Whether this is a key-down event; key-up events carry no new input.
    pub(crate) down: bool,
    /// The UTF-16 code unit the console reports for the key.
    pub(crate) character: u16,
    /// How many times the key event repeats.
    pub(crate) repeat: u16,
}

/// One console input record, reduced to what a query reply cares about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// A character key event.
    Key(KeyEvent),
    /// Buttons, focus, buffer resize, menu, or any other record.
    Other,
}

impl Event {
    /// Reduce one console record to an event.
    ///
    /// `is_key` says whether the record is a key event; the remaining fields
    /// come from its key record.
    pub(crate) fn from_key_record(is_key: bool, down: bool, character: u16, repeat: u16) -> Self {
        if is_key {
            Self::Key(KeyEvent {
                down,
                character,
                repeat,
            })
        } else {
            Self::Other
        }
    }
}

/// A non-blocking source of console input events.
pub(crate) trait EventSource {
    /// Return the events currently queued, or an empty vector when none are.
    ///
    /// Implementations must not block forever: when the queue is empty they
    /// sleep for a short poll interval and return. That interval is how quickly
    /// a quiet queue is detected once the reply is complete.
    fn poll(&mut self) -> io::Result<Vec<Event>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_records_become_key_events() {
        assert_eq!(
            Event::from_key_record(true, true, u16::from(b'x'), 2),
            Event::Key(KeyEvent {
                down: true,
                character: u16::from(b'x'),
                repeat: 2,
            })
        );
    }

    #[test]
    fn other_records_stay_other() {
        assert_eq!(
            Event::from_key_record(false, true, u16::from(b'x'), 2),
            Event::Other
        );
    }
}
