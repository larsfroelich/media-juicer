use std::path::Path;
use std::process::ExitCode;

use media_juicer::app::execute::{ExecutionError, execute_plan};
use media_juicer::error::MediaJuicerError;
use media_juicer::image_processing::SystemImageBackend;
use media_juicer::planning::build_processing_plan;
use media_juicer::selection::Mode;
use media_juicer::timestamps::FileSystemTimestampProvider;
use media_juicer::video_processing::{StdFileSizeProvider, SystemFfmpegExecutor};

const EXIT_SUCCESS: u8 = 0;
const EXIT_PARTIAL_FAILURE: u8 = 1;
const EXIT_INVALID_INPUT: u8 = 2;

fn main() -> ExitCode {
    let config = match media_juicer::cli::parse_args() {
        Ok(config) => config,
        Err(error) => error.exit(),
    };

    let source_path = Path::new(&config.folder_path);
    if !(source_path.exists() && source_path.is_dir()) {
        eprintln!(
            "invalid input: source folder does not exist or is not a directory: {}",
            source_path.display()
        );
        return ExitCode::from(EXIT_INVALID_INPUT);
    }

    let plan =
        match build_processing_plan(source_path, map_mode(config.mode), config.only.as_deref()) {
            Ok(plan) => plan,
            Err(err) => {
                eprintln!("{err}");
                return match err {
                    MediaJuicerError::InvalidInput(_) => ExitCode::from(EXIT_INVALID_INPUT),
                    MediaJuicerError::Io(_) => ExitCode::from(EXIT_PARTIAL_FAILURE),
                };
            }
        };

    let image_backend = SystemImageBackend;
    let ffmpeg_executor = SystemFfmpegExecutor;
    let size_provider = StdFileSizeProvider;
    let timestamps = FileSystemTimestampProvider;

    let mut stdout = std::io::stdout().lock();
    let result = execute_plan(
        &plan,
        &config,
        &image_backend,
        &ffmpeg_executor,
        &size_provider,
        &timestamps,
        &mut stdout,
    );

    let processed_count = match &result {
        Ok(summary) => summary.progress.processed_files,
        Err(ExecutionError::FileFailures(summary)) => summary.progress.processed_files,
        Err(ExecutionError::ReportIo(_)) => 0,
    };
    println!("Processed a total of {processed_count} files.");

    match result {
        Ok(_) => ExitCode::from(EXIT_SUCCESS),
        Err(ExecutionError::FileFailures(_)) => ExitCode::from(EXIT_PARTIAL_FAILURE),
        Err(ExecutionError::ReportIo(err)) => {
            eprintln!("{err}");
            ExitCode::from(EXIT_PARTIAL_FAILURE)
        }
    }
}

fn map_mode(mode: media_juicer::config::ProcessingMode) -> Mode {
    match mode {
        media_juicer::config::ProcessingMode::All => Mode::All,
        media_juicer::config::ProcessingMode::Videos => Mode::Videos,
        media_juicer::config::ProcessingMode::Images => Mode::Images,
        media_juicer::config::ProcessingMode::FixDates => Mode::Fixdates,
    }
}
