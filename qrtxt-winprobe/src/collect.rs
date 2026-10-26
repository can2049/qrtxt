//! The safe polling loop shared by every platform.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::event::{Event, EventSource};
use crate::utf16::Utf16Decoder;

/// Write `queries` in order and collect the replies.
pub(crate) fn transceive<S, W>(
    source: &mut S,
    writer: &mut W,
    queries: &[&[u8]],
    timeout: Duration,
    done: &dyn Fn(&[u8]) -> bool,
) -> io::Result<Vec<u8>>
where
    S: EventSource,
    W: Write,
{
    for query in queries {
        writer.write_all(query)?;
    }
    writer.flush()?;
    collect(source, timeout, done)
}

/// Read until `done` accepts the reply (then drain trailing events) or the
/// deadline passes.
fn collect<S>(
    source: &mut S,
    timeout: Duration,
    done: &dyn Fn(&[u8]) -> bool,
) -> io::Result<Vec<u8>>
where
    S: EventSource,
{
    let deadline = Instant::now() + timeout;
    let mut decoder = Utf16Decoder::default();
    let mut response = Vec::new();
    let mut settled = false;
    loop {
        if Instant::now() >= deadline {
            break;
        }
        let events = source.poll()?;
        if events.is_empty() {
            // A quiet queue is the end of the reply once the verdict is final;
            // before that, keep waiting for the first answer.
            if settled {
                break;
            }
            continue;
        }
        for event in events {
            if let Event::Key(key) = event {
                if key.down && key.character != 0 {
                    decoder.push(key.character, key.repeat, &mut response);
                }
            }
        }
        if !settled && done(&response) {
            settled = true;
        }
    }
    decoder.finish(&mut response);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::KeyEvent;
    use std::collections::VecDeque;

    struct ScriptedSource {
        batches: VecDeque<Vec<Event>>,
        error: Option<io::ErrorKind>,
    }

    impl ScriptedSource {
        fn new(batches: Vec<Vec<Event>>) -> Self {
            Self {
                batches: batches.into(),
                error: None,
            }
        }

        fn failing(kind: io::ErrorKind) -> Self {
            Self {
                batches: VecDeque::new(),
                error: Some(kind),
            }
        }
    }

    impl EventSource for ScriptedSource {
        fn poll(&mut self) -> io::Result<Vec<Event>> {
            if let Some(kind) = self.error {
                return Err(io::Error::from(kind));
            }
            if let Some(batch) = self.batches.pop_front() {
                return Ok(batch);
            }
            std::thread::sleep(Duration::from_millis(1));
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct RecordingWriter {
        written: Vec<u8>,
        flushed: bool,
    }

    impl Write for RecordingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushed = true;
            Ok(())
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn key(unit: u16) -> Event {
        Event::Key(KeyEvent {
            down: true,
            character: unit,
            repeat: 1,
        })
    }

    fn char_key(character: char) -> Event {
        key(u16::try_from(u32::from(character)).expect("BMP test character"))
    }

    fn repeated(unit: u16, repeat: u16) -> Event {
        Event::Key(KeyEvent {
            down: true,
            character: unit,
            repeat,
        })
    }

    fn key_up(unit: u16) -> Event {
        Event::Key(KeyEvent {
            down: false,
            character: unit,
            repeat: 1,
        })
    }

    fn collect_from(
        batches: Vec<Vec<Event>>,
        timeout: Duration,
        done: &dyn Fn(&[u8]) -> bool,
    ) -> (Vec<u8>, Duration) {
        let mut source = ScriptedSource::new(batches);
        let start = Instant::now();
        let response = collect(&mut source, timeout, done).expect("collect should succeed");
        (response, start.elapsed())
    }

    #[test]
    fn transceive_writes_queries_in_order_then_collects() {
        let mut source = ScriptedSource::new(vec![vec![char_key('r')]]);
        let mut writer = RecordingWriter::default();
        let reply = transceive(
            &mut source,
            &mut writer,
            &[b"one", b"two"],
            Duration::from_millis(200),
            &|bytes: &[u8]| bytes == b"r",
        )
        .expect("transceive should succeed");
        assert_eq!(writer.written, b"onetwo");
        assert!(writer.flushed);
        assert_eq!(reply, b"r");
    }

    #[test]
    fn collects_key_down_characters_in_order() {
        let (reply, elapsed) = collect_from(
            vec![vec![char_key('o'), char_key('k')]],
            Duration::from_millis(500),
            &|bytes: &[u8]| bytes == b"ok",
        );
        assert_eq!(reply, b"ok");
        assert!(
            elapsed < Duration::from_millis(300),
            "a completed reply should not wait for the deadline: {elapsed:?}"
        );
    }

    #[test]
    fn ignores_key_up_non_key_and_nul_events() {
        let (reply, _) = collect_from(
            vec![vec![
                key_up(u16::from(b'x')),
                Event::Other,
                key(0),
                char_key('y'),
            ]],
            Duration::from_millis(40),
            &|_: &[u8]| false,
        );
        assert_eq!(reply, b"y");
    }

    #[test]
    fn honours_repeat_counts() {
        let (reply, _) = collect_from(
            vec![vec![
                repeated(u16::from(b'a'), 3),
                repeated(u16::from(b'b'), 0),
            ]],
            Duration::from_millis(40),
            &|_: &[u8]| false,
        );
        assert_eq!(reply, b"aaab");
    }

    #[test]
    fn decodes_surrogate_pairs() {
        let (reply, _) = collect_from(
            vec![vec![key(0xD83D), key(0xDE00)]],
            Duration::from_millis(40),
            &|_: &[u8]| false,
        );
        assert_eq!(reply, "😀".as_bytes());
    }

    #[test]
    fn a_lone_high_surrogate_is_replaced_at_the_deadline() {
        let (reply, elapsed) = collect_from(
            vec![vec![key(0xD83D)]],
            Duration::from_millis(30),
            &|_: &[u8]| false,
        );
        assert_eq!(reply, "\u{FFFD}".as_bytes());
        assert!(elapsed >= Duration::from_millis(20));
    }

    #[test]
    fn drains_trailing_events_after_done() {
        let (reply, elapsed) = collect_from(
            vec![vec![char_key('o'), char_key('k')], vec![char_key('!')]],
            Duration::from_millis(500),
            &|bytes: &[u8]| bytes.ends_with(b"ok"),
        );
        assert_eq!(reply, b"ok!");
        assert!(
            elapsed < Duration::from_millis(300),
            "draining should stop at the first quiet poll: {elapsed:?}"
        );
    }

    #[test]
    fn returns_a_partial_reply_at_the_deadline() {
        let (reply, elapsed) = collect_from(
            vec![vec![char_key('p')]],
            Duration::from_millis(40),
            &|_: &[u8]| false,
        );
        assert_eq!(reply, b"p");
        assert!(elapsed >= Duration::from_millis(30));
    }

    #[test]
    fn returns_an_empty_reply_when_nothing_arrives() {
        let (reply, elapsed) = collect_from(vec![], Duration::from_millis(40), &|_: &[u8]| false);
        assert!(reply.is_empty());
        assert!(elapsed >= Duration::from_millis(30));
    }

    #[test]
    fn propagates_source_errors() {
        let mut source = ScriptedSource::failing(io::ErrorKind::BrokenPipe);
        let error = collect(&mut source, Duration::from_millis(100), &|_: &[u8]| false)
            .expect_err("the source error must surface");
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn propagates_write_errors() {
        let mut source = ScriptedSource::new(vec![]);
        let error = transceive(
            &mut source,
            &mut FailingWriter,
            &[b"query"],
            Duration::from_millis(10),
            &|_: &[u8]| false,
        )
        .expect_err("the write error must surface");
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_zero_timeout_returns_immediately() {
        let (reply, elapsed) =
            collect_from(vec![vec![char_key('x')]], Duration::ZERO, &|_: &[u8]| false);
        assert!(reply.is_empty());
        assert!(elapsed < Duration::from_millis(100));
    }
}
