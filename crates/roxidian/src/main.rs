//! `rox` opens a readonly Markdown workspace without changing the invocation directory.

mod cli;
mod document;
mod native;
mod render;
mod search;
mod syntax;
mod theme;
mod ui;
mod workspace;

#[cfg(test)]
mod tests;

fn main() {
    use clap::Parser;
    let args = cli::Arguments::parse();
    let launch = std::env::current_dir()
        .map_err(anyhow::Error::from)
        .and_then(|cwd| cli::Launch::resolve(&cwd, args.path.as_deref()));
    match launch {
        Ok(launch) => ui::run(launch, args.theme),
        Err(error) => {
            eprintln!("rox: {error:#}");
            std::process::exit(1);
        }
    }
}
