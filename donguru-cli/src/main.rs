use donguru_cli::{cli, commands, completion, context, exit};

fn main() -> std::process::ExitCode {
    completion::complete_if_asked();
    let cli = cli::Cli::parse_styled();
    let ctx = context::Context::from_cli(&cli);

    let exit_code = match commands::dispatch(&ctx, cli.command) {
        Ok(code) => code,
        // `dg power status | head -2`: a reader that went away is not an error (CLI-48)
        Err(err) if exit::is_broken_pipe(&err) => exit::ExitCode::Success,
        Err(err) => {
            eprintln!("donguru: error: {err:#}");
            exit::ExitCode::from(&err)
        }
    };
    exit_code.into()
}
