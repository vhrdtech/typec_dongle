mod cli;
mod commands;
mod context;
mod exit;
mod theme;

fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse_styled();
    let ctx = context::Context::from_cli(&cli);

    let exit_code = match commands::dispatch(&ctx, cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("donguru: error: {err:#}");
            exit::ExitCode::from(&err)
        }
    };
    exit_code.into()
}
