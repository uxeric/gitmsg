mod auth;
mod grok;
mod spinner;
mod staged;
mod style;
mod subject;
mod typeout;
mod update;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", style::failure(&error, style::stderr_color()));
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    if let Some(pending) = args.type_when_ready {
        typeout::type_when_ready(&pending.address, &pending.command);
        return Ok(());
    }
    if args.version {
        let rev =
            update::installed_rev().unwrap_or_else(|| "not installed by install.sh".to_string());
        println!(
            "{}",
            style::version_line(env!("CARGO_PKG_VERSION"), &rev, style::stdout_color())
        );
        return Ok(());
    }
    if args.update {
        let status = update::run_installer().map_err(|error| {
            format!("could not run the installer ({error}); nothing was updated.")
        })?;
        if !status.success() {
            return Err(
                "the update did not finish; your previous version is still installed.".to_string(),
            );
        }
        return Ok(());
    }
    if args.help {
        print_help();
        return Ok(());
    }

    update::offer_if_newer()?;

    let spinner = spinner::Spinner::start("Generating");
    let staged = staged::collect()?;
    let subject = grok::draft_subject(&staged.body)?;
    drop(spinner);
    let command = subject::git_commit_invocation(&subject);
    typeout::deliver(&command, args.print_only);
    Ok(())
}

struct Args {
    help: bool,
    version: bool,
    update: bool,
    print_only: bool,
    type_when_ready: Option<PendingType>,
}

struct PendingType {
    address: String,
    command: String,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut help = false;
        let mut version = false;
        let mut update = false;
        let mut print_only = false;
        let mut type_address: Option<String> = None;
        let mut command: Option<String> = None;
        let mut raw = env::args().skip(1);
        while let Some(arg) = raw.next() {
            match arg.as_str() {
                "-h" | "--help" => help = true,
                "-V" | "--version" => version = true,
                "--update" => update = true,
                "--print" => print_only = true,
                "--type-when-ready" => {
                    let address = raw
                        .next()
                        .ok_or_else(|| "--type-when-ready needs the window address.".to_string())?;
                    type_address = Some(address);
                }
                "--" => {
                    command = raw.next();
                    break;
                }
                other if other.starts_with('-') => {
                    return Err(format!("Unknown option `{other}`. Run `gitmsg --help`."));
                }
                other => {
                    return Err(format!(
                        "Unexpected argument `{other}`. Run `gitmsg --help`."
                    ));
                }
            }
        }
        let type_when_ready = match type_address {
            Some(address) => Some(PendingType {
                address,
                command: command
                    .ok_or_else(|| "--type-when-ready needs the command after --.".to_string())?,
            }),
            None => None,
        };
        Ok(Self {
            help,
            version,
            update,
            print_only,
            type_when_ready,
        })
    }
}

fn print_help() {
    println!(
        "\
gitmsg — draft a commit title for what is staged and type it into the terminal

Usage:
  gitmsg            Type this on the prompt: git commit -m \"<title>\"
  gitmsg --print    Print that command instead of typing it
  gitmsg --update   Download and install the latest version
  gitmsg --version  Show the installed version

The title comes from the staged diff and your Grok login (~/.grok/auth.json).
Nothing is committed. Read the line, then press Enter.

Before drafting a title, gitmsg checks for a newer version. Enter installs it
and continues. n keeps this version.

The title starts with a verb such as Adds, Updates, or Refactors. It aims for
50 characters, stays within 72, and uses & instead of \"and\". There is no
commit body.

GITMSG_MODEL            Model id. Defaults to the model in ~/.grok/config.toml.
GITMSG_EFFORT           Reasoning effort. Defaults to low.
GITMSG_NO_UPDATE_CHECK  Set to skip the update check."
    );
}
