use std::io::IsTerminal;

fn main() -> color_eyre::Result<()> {
    match skillroom::cli::decide(
        std::env::args().skip(1),
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    ) {
        skillroom::cli::CliAction::RunTui => skillroom::run(),
        skillroom::cli::CliAction::PrintVersion => {
            println!("{}", skillroom::cli::version_line());
            Ok(())
        }
        skillroom::cli::CliAction::PrintNonInteractiveNotice => {
            println!("{}", skillroom::cli::non_interactive_notice());
            Ok(())
        }
    }
}
