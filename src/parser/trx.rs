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

    // Current test being parsed
    let mut current_test: Option<TestResult> = None;
    let mut in_error_info = false;
    let mut in_message = false;
    let mut in_stack_trace = false;
    let mut error_message = String::new();
    let mut stack_trace = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"UnitTestResult" => {
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
                            current_test = Some(TestResult {
                                test_name,
                                outcome,
                                duration_ms,
                                error_message: None,
                            });
                        }
                    }
                    b"ErrorInfo" => in_error_info = true,
                    b"Message" if in_error_info => in_message = true,
                    b"StackTrace" if in_error_info => in_stack_trace = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"UnitTestResult" => {
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
            Ok(Event::Text(e)) => {
                if in_message {
                    error_message.push_str(&e.unescape().unwrap_or_default());
                } else if in_stack_trace {
                    stack_trace.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"UnitTestResult" => {
                        if let Some(mut test) = current_test.take() {
                            // Combine error message and stack trace
                            if !error_message.is_empty() || !stack_trace.is_empty() {
                                let mut full_error = error_message.trim().to_string();
                                if !stack_trace.is_empty() {
                                    if !full_error.is_empty() {
                                        full_error.push_str("\n\n");
                                    }
                                    full_error.push_str(stack_trace.trim());
                                }
                                test.error_message = Some(full_error);
                            }
                            results.push(test);
                            error_message.clear();
                            stack_trace.clear();
                        }
                    }
                    b"ErrorInfo" => in_error_info = false,
                    b"Message" => in_message = false,
                    b"StackTrace" => in_stack_trace = false,
                    _ => {}
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
        let frac = secs_parts[1].as_bytes();
        let digit = |i: usize| -> u64 {
            frac.get(i)
                .filter(|b| b.is_ascii_digit())
                .map(|b| (b - b'0') as u64)
                .unwrap_or(0)
        };
        digit(0) * 100 + digit(1) * 10 + digit(2)
    } else {
        0
    };

    (hours * 3600 + minutes * 60 + seconds) * 1000 + millis
}

#[cfg(test)]
mod tests {
    use super::*;

    // TestOutcome tests
    #[test]
    fn test_outcome_equality() {
        assert_eq!(TestOutcome::Passed, TestOutcome::Passed);
        assert_eq!(TestOutcome::Failed, TestOutcome::Failed);
        assert_eq!(TestOutcome::Skipped, TestOutcome::Skipped);
    }

    #[test]
    fn test_outcome_inequality() {
        assert_ne!(TestOutcome::Passed, TestOutcome::Failed);
        assert_ne!(TestOutcome::Failed, TestOutcome::Skipped);
        assert_ne!(TestOutcome::Passed, TestOutcome::Skipped);
    }

    #[test]
    fn test_outcome_clone() {
        let outcome = TestOutcome::Passed;
        let cloned = outcome.clone();
        assert_eq!(outcome, cloned);
    }

    #[test]
    fn test_outcome_debug() {
        assert_eq!(format!("{:?}", TestOutcome::Passed), "Passed");
        assert_eq!(format!("{:?}", TestOutcome::Failed), "Failed");
        assert_eq!(format!("{:?}", TestOutcome::Skipped), "Skipped");
    }

    // parse_duration tests
    #[test]
    fn test_parse_duration_zero() {
        assert_eq!(parse_duration("00:00:00.0000000"), 0);
        assert_eq!(parse_duration("0:0:0.0"), 0);
    }

    #[test]
    fn test_parse_duration_milliseconds_only() {
        assert_eq!(parse_duration("00:00:00.0010000"), 1);
        assert_eq!(parse_duration("00:00:00.0100000"), 10);
        assert_eq!(parse_duration("00:00:00.1000000"), 100);
        assert_eq!(parse_duration("00:00:00.5000000"), 500);
        assert_eq!(parse_duration("00:00:00.9990000"), 999);
    }

    #[test]
    fn test_parse_duration_seconds_only() {
        assert_eq!(parse_duration("00:00:01.0000000"), 1000);
        assert_eq!(parse_duration("00:00:30.0000000"), 30000);
        assert_eq!(parse_duration("00:00:59.0000000"), 59000);
    }

    #[test]
    fn test_parse_duration_minutes_only() {
        assert_eq!(parse_duration("00:01:00.0000000"), 60000);
        assert_eq!(parse_duration("00:30:00.0000000"), 1800000);
        assert_eq!(parse_duration("00:59:00.0000000"), 3540000);
    }

    #[test]
    fn test_parse_duration_hours_only() {
        assert_eq!(parse_duration("01:00:00.0000000"), 3600000);
        assert_eq!(parse_duration("02:00:00.0000000"), 7200000);
        assert_eq!(parse_duration("10:00:00.0000000"), 36000000);
    }

    #[test]
    fn test_parse_duration_combined() {
        // 1h 30m 45s 123ms
        assert_eq!(parse_duration("01:30:45.1230000"), 5445123);
        // 2h 15m 30s 500ms
        assert_eq!(parse_duration("02:15:30.5000000"), 8130500);
    }

    #[test]
    fn test_parse_duration_short_fraction() {
        assert_eq!(parse_duration("00:00:01.1"), 1100);
        assert_eq!(parse_duration("00:00:01.12"), 1120);
        assert_eq!(parse_duration("00:00:01.123"), 1123);
    }

    #[test]
    fn test_parse_duration_no_fraction() {
        assert_eq!(parse_duration("00:00:05"), 5000);
        assert_eq!(parse_duration("01:30:00"), 5400000);
    }

    #[test]
    fn test_parse_duration_invalid_format() {
        assert_eq!(parse_duration(""), 0);
        assert_eq!(parse_duration("invalid"), 0);
        assert_eq!(parse_duration("00:00"), 0);
        assert_eq!(parse_duration("00"), 0);
        assert_eq!(parse_duration("00:00:00:00"), 0);
    }

    #[test]
    fn test_parse_duration_non_numeric_parts() {
        assert_eq!(parse_duration("aa:00:00.0"), 0);
        assert_eq!(parse_duration("00:bb:00.0"), 0);
        assert_eq!(parse_duration("00:00:cc.0"), 0);
    }

    // parse_trx tests
    #[test]
    fn test_parse_trx_empty_content() {
        let result = parse_trx("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_trx_no_test_results() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <TestSettings />
            </TestRun>"#;
        let result = parse_trx(xml).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_trx_single_passed_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="MyNamespace.MyClass.TestMethod1" outcome="Passed" duration="00:00:01.1234567" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].test_name, "MyNamespace.MyClass.TestMethod1");
        assert_eq!(result[0].outcome, TestOutcome::Passed);
        assert_eq!(result[0].duration_ms, 1123);
    }

    #[test]
    fn test_parse_trx_single_failed_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="FailedTest" outcome="Failed" duration="00:00:00.5000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].test_name, "FailedTest");
        assert_eq!(result[0].outcome, TestOutcome::Failed);
        assert_eq!(result[0].duration_ms, 500);
    }

    #[test]
    fn test_parse_trx_skipped_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="SkippedTest" outcome="NotExecuted" duration="00:00:00.0000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].outcome, TestOutcome::Skipped);
    }

    #[test]
    fn test_parse_trx_multiple_tests() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" outcome="Passed" duration="00:00:00.1000000" />
                    <UnitTestResult testName="Test2" outcome="Failed" duration="00:00:00.2000000" />
                    <UnitTestResult testName="Test3" outcome="Passed" duration="00:00:00.3000000" />
                    <UnitTestResult testName="Test4" outcome="NotExecuted" duration="00:00:00.0000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 4);

        assert_eq!(result[0].test_name, "Test1");
        assert_eq!(result[0].outcome, TestOutcome::Passed);
        assert_eq!(result[0].duration_ms, 100);

        assert_eq!(result[1].test_name, "Test2");
        assert_eq!(result[1].outcome, TestOutcome::Failed);
        assert_eq!(result[1].duration_ms, 200);

        assert_eq!(result[2].test_name, "Test3");
        assert_eq!(result[2].outcome, TestOutcome::Passed);
        assert_eq!(result[2].duration_ms, 300);

        assert_eq!(result[3].test_name, "Test4");
        assert_eq!(result[3].outcome, TestOutcome::Skipped);
    }

    #[test]
    fn test_parse_trx_missing_attributes() {
        // Missing testName - should skip this result
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult outcome="Passed" duration="00:00:00.1000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_trx_missing_outcome_defaults_to_passed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" duration="00:00:00.1000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].outcome, TestOutcome::Passed);
    }

    #[test]
    fn test_parse_trx_missing_duration_defaults_to_zero() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" outcome="Passed" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].duration_ms, 0);
    }

    #[test]
    fn test_parse_trx_unknown_outcome_treated_as_skipped() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" outcome="SomeUnknownStatus" duration="00:00:00.1000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result[0].outcome, TestOutcome::Skipped);
    }

    #[test]
    fn test_parse_trx_with_start_element() {
        // UnitTestResult with content inside (Start element instead of Empty)
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" outcome="Failed" duration="00:00:01.0000000">
                        <Output>
                            <ErrorInfo>
                                <Message>Test failed</Message>
                            </ErrorInfo>
                        </Output>
                    </UnitTestResult>
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].test_name, "Test1");
        assert_eq!(result[0].outcome, TestOutcome::Failed);
    }

    #[test]
    fn test_parse_trx_malformed_xml() {
        let xml = r#"<TestRun><Results><UnitTestResult"#;
        let result = parse_trx(xml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_trx_complex_test_names() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Company.Product.Tests.Integration.DatabaseTests.Should_Insert_Record_When_Valid" outcome="Passed" duration="00:00:02.5000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result[0].test_name, "Company.Product.Tests.Integration.DatabaseTests.Should_Insert_Record_When_Valid");
    }

    #[test]
    fn test_parse_trx_test_name_with_special_chars() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test_With_Underscore" outcome="Passed" duration="00:00:00.0010000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert_eq!(result[0].test_name, "Test_With_Underscore");
    }

    #[test]
    fn test_result_error_message_is_none() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <TestRun>
                <Results>
                    <UnitTestResult testName="Test1" outcome="Passed" duration="00:00:00.1000000" />
                </Results>
            </TestRun>"#;

        let result = parse_trx(xml).unwrap();
        assert!(result[0].error_message.is_none());
    }

    #[test]
    fn test_result_clone() {
        let result = TestResult {
            test_name: "Test1".to_string(),
            outcome: TestOutcome::Passed,
            duration_ms: 100,
            error_message: Some("error".to_string()),
        };

        let cloned = result.clone();
        assert_eq!(cloned.test_name, "Test1");
        assert_eq!(cloned.outcome, TestOutcome::Passed);
        assert_eq!(cloned.duration_ms, 100);
        assert_eq!(cloned.error_message, Some("error".to_string()));
    }

    #[test]
    fn test_result_debug() {
        let result = TestResult {
            test_name: "Test1".to_string(),
            outcome: TestOutcome::Failed,
            duration_ms: 100,
            error_message: None,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Test1"));
        assert!(debug_str.contains("Failed"));
    }
}
