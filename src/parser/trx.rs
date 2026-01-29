use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Result, TestamentError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_name: String,
    pub outcome: TestOutcome,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

pub fn parse_trx(content: &str) -> Result<Vec<TestResult>> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"UnitTestResult" => {
                let mut test_name = String::new();
                let mut outcome = TestOutcome::Passed;
                let mut duration_ms = 0u64;

                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"testName" => {
                            test_name = String::from_utf8_lossy(&attr.value).to_string();
                        }
                        b"outcome" => {
                            let val = String::from_utf8_lossy(&attr.value);
                            outcome = match val.as_ref() {
                                "Passed" => TestOutcome::Passed,
                                "Failed" => TestOutcome::Failed,
                                _ => TestOutcome::Skipped,
                            };
                        }
                        b"duration" => {
                            let val = String::from_utf8_lossy(&attr.value);
                            duration_ms = parse_duration(&val);
                        }
                        _ => {}
                    }
                }

                if !test_name.is_empty() {
                    results.push(TestResult {
                        test_name,
                        outcome,
                        duration_ms,
                        error_message: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TestamentError::TrxParse(format!(
                    "XML parse error: {}",
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(results)
}

fn parse_duration(s: &str) -> u64 {
    // Format: HH:MM:SS.FFFFFFF
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return 0;
    }

    let hours: u64 = parts[0].parse().unwrap_or(0);
    let minutes: u64 = parts[1].parse().unwrap_or(0);
    let secs_parts: Vec<&str> = parts[2].split('.').collect();
    let seconds: u64 = secs_parts[0].parse().unwrap_or(0);
    let millis: u64 = if secs_parts.len() > 1 {
        let frac = secs_parts[1];
        let frac_padded = format!("{:0<7}", frac);
        frac_padded[..3].parse().unwrap_or(0)
    } else {
        0
    };

    (hours * 3600 + minutes * 60 + seconds) * 1000 + millis
}
