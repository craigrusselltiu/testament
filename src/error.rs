use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestamentError {
    #[error("No solution file found")]
    NoSolutionFound,

    #[error("Failed to read file: {path}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse solution file: {0}")]
    SolutionParse(String),

    #[error("Failed to run dotnet: {0}")]
    DotnetExecution(String),

    #[error("Failed to parse TRX file: {0}")]
    TrxParse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TestamentError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    // NoSolutionFound tests
    #[test]
    fn test_no_solution_found_display() {
        let error = TestamentError::NoSolutionFound;
        assert_eq!(format!("{}", error), "No solution file found");
    }

    #[test]
    fn test_no_solution_found_debug() {
        let error = TestamentError::NoSolutionFound;
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("NoSolutionFound"));
    }

    // FileRead tests
    #[test]
    fn test_file_read_display() {
        let io_error = IoError::new(ErrorKind::NotFound, "file not found");
        let error = TestamentError::FileRead {
            path: PathBuf::from("/path/to/file.txt"),
            source: io_error,
        };
        let display = format!("{}", error);
        assert!(display.contains("Failed to read file"));
        assert!(display.contains("/path/to/file.txt"));
    }

    #[test]
    fn test_file_read_debug() {
        let io_error = IoError::new(ErrorKind::NotFound, "file not found");
        let error = TestamentError::FileRead {
            path: PathBuf::from("/test/path"),
            source: io_error,
        };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("FileRead"));
        assert!(debug_str.contains("path"));
    }

    #[test]
    fn test_file_read_source() {
        let io_error = IoError::new(ErrorKind::PermissionDenied, "permission denied");
        let error = TestamentError::FileRead {
            path: PathBuf::from("/protected/file"),
            source: io_error,
        };

        // The #[source] attribute allows us to get the underlying error
        use std::error::Error;
        let source = error.source();
        assert!(source.is_some());
    }

    // SolutionParse tests
    #[test]
    fn test_solution_parse_display() {
        let error = TestamentError::SolutionParse("Invalid format".to_string());
        assert_eq!(format!("{}", error), "Failed to parse solution file: Invalid format");
    }

    #[test]
    fn test_solution_parse_debug() {
        let error = TestamentError::SolutionParse("Parse error".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("SolutionParse"));
        assert!(debug_str.contains("Parse error"));
    }

    #[test]
    fn test_solution_parse_empty_message() {
        let error = TestamentError::SolutionParse(String::new());
        assert_eq!(format!("{}", error), "Failed to parse solution file: ");
    }

    // DotnetExecution tests
    #[test]
    fn test_dotnet_execution_display() {
        let error = TestamentError::DotnetExecution("Command failed".to_string());
        assert_eq!(format!("{}", error), "Failed to run dotnet: Command failed");
    }

    #[test]
    fn test_dotnet_execution_debug() {
        let error = TestamentError::DotnetExecution("Exit code 1".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("DotnetExecution"));
        assert!(debug_str.contains("Exit code 1"));
    }

    #[test]
    fn test_dotnet_execution_with_stderr() {
        let stderr = "Error: Test project not found\nBuild failed with 3 errors";
        let error = TestamentError::DotnetExecution(stderr.to_string());
        let display = format!("{}", error);
        assert!(display.contains("Test project not found"));
    }

    // TrxParse tests
    #[test]
    fn test_trx_parse_display() {
        let error = TestamentError::TrxParse("XML error".to_string());
        assert_eq!(format!("{}", error), "Failed to parse TRX file: XML error");
    }

    #[test]
    fn test_trx_parse_debug() {
        let error = TestamentError::TrxParse("Malformed XML".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("TrxParse"));
        assert!(debug_str.contains("Malformed XML"));
    }

    // Io tests
    #[test]
    fn test_io_display() {
        let io_error = IoError::new(ErrorKind::ConnectionRefused, "connection refused");
        let error = TestamentError::Io(io_error);
        let display = format!("{}", error);
        assert!(display.contains("IO error"));
        assert!(display.contains("connection refused"));
    }

    #[test]
    fn test_io_from_conversion() {
        let io_error = IoError::new(ErrorKind::BrokenPipe, "broken pipe");
        let error: TestamentError = io_error.into();
        match error {
            TestamentError::Io(_) => (),
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_io_debug() {
        let io_error = IoError::new(ErrorKind::TimedOut, "timed out");
        let error = TestamentError::Io(io_error);
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Io"));
    }

    // Result type alias tests
    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let result: Result<i32> = Err(TestamentError::NoSolutionFound);
        assert!(result.is_err());
    }

    #[test]
    fn test_result_with_question_mark() {
        fn may_fail() -> Result<()> {
            Err(TestamentError::NoSolutionFound)
        }

        fn caller() -> Result<String> {
            may_fail()?;
            Ok("success".to_string())
        }

        let result = caller();
        assert!(result.is_err());
    }

    // Error trait implementation
    #[test]
    fn test_error_trait_impl() {
        use std::error::Error;

        let error = TestamentError::NoSolutionFound;
        let _ = &error as &dyn Error;
    }

    #[test]
    fn test_different_error_variants() {
        let errors: Vec<TestamentError> = vec![
            TestamentError::NoSolutionFound,
            TestamentError::SolutionParse("test".to_string()),
            TestamentError::DotnetExecution("test".to_string()),
            TestamentError::TrxParse("test".to_string()),
            TestamentError::Io(IoError::new(ErrorKind::Other, "test")),
        ];

        for error in errors {
            let _ = format!("{}", error);
            let _ = format!("{:?}", error);
        }
    }
}
