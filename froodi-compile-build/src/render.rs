//! Turning syntax back into text.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use quote::ToTokens;

/// A compact spelling of `node` with whitespace only where Rust needs it, so that
/// `Inject< Config >` and `Inject<Config>` render the same.
pub(crate) fn spelling(node: &impl ToTokens) -> String {
    let mut out = String::new();
    write_stream(node.to_token_stream(), &mut out);
    out
}

/// The source text under `span`, or [`spelling`] of `node` when the span carries no text.
pub(crate) fn source(node: &impl ToTokens, span: Span) -> String {
    span.source_text().unwrap_or_else(|| spelling(node))
}

fn write_stream(tokens: TokenStream, out: &mut String) {
    let mut after_word = false;
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => {
                if after_word {
                    out.push(' ');
                }
                out.push_str(&ident.to_string());
                after_word = true;
            }
            TokenTree::Literal(literal) => {
                if after_word {
                    out.push(' ');
                }
                out.push_str(&literal.to_string());
                after_word = true;
            }
            TokenTree::Punct(punct) => {
                out.push(punct.as_char());
                if punct.as_char() == ',' {
                    out.push(' ');
                }
                after_word = false;
            }
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                write_stream(group.stream(), out);
                out.push_str(close);
                after_word = false;
            }
        }
    }
}
