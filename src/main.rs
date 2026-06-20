use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;

use ferrisetw::parser::Parser;
use ferrisetw::provider::kernel_providers::PROCESS_PROVIDER;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::KernelTrace;
use ferrisetw::EventRecord;

use process_logger::Filters;

// Opcode 1 == process Start in the NT Kernel Logger process provider
// (2 = End, 3 = DCStart, 4 = DCEnd / rundown of already-running processes).
const OPCODE_PROCESS_START: u8 = 1;

const LOG_FILE_NAME: &str = "process-start-log.txt";
const SESSION_NAME: &str = "ProcessLoggerKernelTrace";

// Opened once, shared across the trace's background callback thread.
static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

// Command-line filters, set once at startup, read from the callback thread.
static FILTERS: OnceLock<Filters> = OnceLock::new();

fn print_usage() {
    println!(
        "Usage: process-logger [--include <text>]... [--exclude <text>]...\n\n\
         Logs the command line of every process that starts to {LOG_FILE_NAME}.\n\n\
         Options (repeatable, matched as case-insensitive partial text):\n  \
         --include <text>   Log only lines containing this text (OR-ed across includes).\n  \
         --exclude <text>   Never log lines containing this text (OR-ed; overrides --include).\n  \
         -h, --help         Show this help.\n\n\
         With no --include, all lines are logged except those matching an --exclude."
    );
}

/// Parse repeatable `--include`/`--exclude` options, supporting both
/// `--include value` and `--include=value` forms. Exits on bad input or --help.
fn parse_filters() -> Filters {
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        let take_value = |inline: Option<&str>, args: &mut dyn Iterator<Item = String>, flag: &str| {
            match inline {
                Some(v) => v.to_string(),
                None => args.next().unwrap_or_else(|| {
                    eprintln!("error: {flag} requires a value");
                    std::process::exit(2);
                }),
            }
        };

        if arg == "-h" || arg == "--help" {
            print_usage();
            std::process::exit(0);
        } else if arg == "--include" || arg.starts_with("--include=") {
            let inline = arg.strip_prefix("--include=");
            includes.push(take_value(inline, &mut args, "--include"));
        } else if arg == "--exclude" || arg.starts_with("--exclude=") {
            let inline = arg.strip_prefix("--exclude=");
            excludes.push(take_value(inline, &mut args, "--exclude"));
        } else {
            eprintln!("error: unexpected argument '{arg}'\n");
            print_usage();
            std::process::exit(2);
        }
    }

    // Filters::new lowercases patterns for case-insensitive matching.
    Filters::new(&includes, &excludes)
}

fn open_log_file() -> std::io::Result<File> {
    // Place the log next to the executable, not the (variable) working dir.
    let mut path = std::env::current_exe()?;
    path.set_file_name(LOG_FILE_NAME);
    OpenOptions::new().create(true).append(true).open(path)
}

fn process_callback(record: &EventRecord, schema_locator: &SchemaLocator) {
    if record.opcode() != OPCODE_PROCESS_START {
        return;
    }

    let schema = match schema_locator.event_schema(record) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to resolve event schema: {e:?}");
            return;
        }
    };

    let parser = Parser::create(record, &schema);
    let cmd: String = parser.try_parse("CommandLine").unwrap_or_default();
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }

    if let Some(filters) = FILTERS.get() {
        if !filters.keep(cmd) {
            return;
        }
    }

    if let Some(lock) = LOG_FILE.get() {
        let mut file = lock.lock().unwrap();
        if let Err(e) = writeln!(file, "{cmd}") {
            eprintln!("failed to write to log file: {e:?}");
        }
    }
}

/// Stop a leftover ETW session of the same name from a previous run that was
/// killed instead of exited cleanly (otherwise starting fails with AlreadyExist).
/// Runs before tracing starts, so this helper process is not itself logged.
fn stop_stale_session() {
    let _ = Command::new("logman")
        .args(["stop", SESSION_NAME, "-ets"])
        .output();
}

fn main() {
    let _ = FILTERS.set(parse_filters());

    let file = open_log_file().expect("failed to open log file next to the executable");
    LOG_FILE
        .set(Mutex::new(file))
        .expect("log file already initialized");

    stop_stale_session();

    let provider = Provider::kernel(&PROCESS_PROVIDER)
        .add_callback(process_callback)
        .build();

    let trace = KernelTrace::new()
        .named(SESSION_NAME.to_string())
        .enable(provider)
        .start_and_process()
        .expect("failed to start kernel trace -- run from an elevated terminal");

    // Stop the trace cleanly on Ctrl+C so the ETW session does not leak.
    static RUNNING: AtomicBool = AtomicBool::new(true);
    ctrlc::set_handler(|| RUNNING.store(false, Ordering::SeqCst))
        .expect("failed to set Ctrl+C handler");

    println!("Logging process command lines to {LOG_FILE_NAME} (Ctrl+C to stop)...");

    // The trace is processed on a background thread; keep main alive.
    while RUNNING.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    trace.stop().expect("failed to stop kernel trace");
    println!("Stopped.");
}
