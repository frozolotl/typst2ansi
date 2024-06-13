use std::{io::Read, path::PathBuf};

use clap::{ArgAction, Parser, ValueEnum};
use color_eyre::eyre::{Context as _, Result};
use typst_ansi_hl::Highlighter;

#[derive(clap::Parser)]
struct Args {
    /// The input path. If unset, stdin is used.
    input: Option<PathBuf>,

    /// Whether the input should be formatted to be Discord-compatible.
    #[clap(short, long)]
    discord: bool,

    // Logically this comes after `Args::strip_ansi`, but in clap it makes more sense before.
    // Also see https://jwodder.github.io/kbits/posts/clap-bool-negate/
    /// Strip all ANSI escape sequences from the input before processing. [default]
    #[clap(short = 's', long = "strip-ansi")]
    #[clap(hide_short_help = true, overrides_with = "strip_ansi")]
    #[doc(hidden)]
    _strip_ansi: bool,

    /// Don't remove escape sequences from the input.
    #[clap(
        action = ArgAction::SetFalse, // This turns the value *off*!
        short = 'S',                  //
        long = "no-strip-ansi",       // so this is not a bug!
    )]
    strip_ansi: bool,

    /// If the input is surrounded by "```" lines, remove them.
    ///
    /// The opening delimiter will be matched even when followed by some non-whitespace 'word'.
    /// The closing delimiter will be matched even when followed by a newline.
    #[clap(short = 'c', long, overrides_with = "_no_unwrap_codeblock")]
    unwrap_codeblock: bool,

    /// Don't remove surrounding "```" from the input. [default]
    #[clap(short = 'C', long = "no-unwrap-codeblock")]
    #[clap(hide_short_help = true)]
    #[doc(hidden)]
    _no_unwrap_codeblock: bool,

    /// Softly enforce a byte size limit.
    ///
    /// This means that if the size limit is exceeded, less colors are used
    /// in order to get below that size limit.
    /// If it is not possible to get below that limit, the text is printed anyway.
    #[clap(short = 'l', long)]
    soft_limit: Option<usize>,

    /// The kind of input syntax.
    #[clap(short, long, default_value = "markup")]
    mode: SyntaxMode,
}

/// The kind of input syntax.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyntaxMode {
    Code,
    Markup,
    Math,
}

impl From<SyntaxMode> for typst_ansi_hl::SyntaxMode {
    fn from(value: SyntaxMode) -> Self {
        match value {
            SyntaxMode::Code => typst_ansi_hl::SyntaxMode::Code,
            SyntaxMode::Markup => typst_ansi_hl::SyntaxMode::Markup,
            SyntaxMode::Math => typst_ansi_hl::SyntaxMode::Math,
        }
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let mut input = String::new();
    if let Some(path) = &args.input {
        std::fs::File::open(path)
            .and_then(|mut f| f.read_to_string(&mut input))
            .wrap_err_with(|| format!("failed to read file `{}`", path.display()))?;
    } else {
        std::io::stdin()
            .read_to_string(&mut input)
            .wrap_err("failed to read from stdin")?;
    }

    let mut stripped = if args.unwrap_codeblock {
        unwrap_codeblock(&input)
    } else {
        &input
    };

    if args.strip_ansi {
        input = strip_ansi_escapes::strip_str(stripped);
        stripped = &input;
    }

    let out = termcolor::Ansi::new(std::io::stdout().lock());
    let mut highlighter = Highlighter::default();
    if args.discord {
        highlighter.for_discord();
    }
    highlighter.with_syntax_mode(args.mode.into());
    if let Some(soft_limit) = args.soft_limit {
        highlighter.with_soft_limit(soft_limit);
    }
    highlighter
        .highlight_to(stripped, out)
        .wrap_err("failed to highlight input")?;

    Ok(())
}

fn unwrap_codeblock(input: &str) -> &str {
    let Some(line_end) = input.find('\n') else {
        return input;
    };
    // Assume as little as possible about the format of language identifier; only that they don't
    // contain whitespace.
    if input.starts_with("```") && input[3..line_end].chars().all(|c| !c.is_whitespace()) {
        for end in ["```", "```\n", "```\r\n"] {
            if input.ends_with(end) {
                return &input[(line_end + 1)..input.len() - end.len()];
            }
        }
    }

    input
}
