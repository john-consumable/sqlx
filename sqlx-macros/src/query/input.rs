use std::fs;

use proc_macro2::{Ident, Span};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, LitBool, LitStr, Token};
use syn::{ExprArray, Type};

/// Macro input shared by `query!()` and `query_file!()`
pub struct QueryMacroInput {
    pub(super) env_var: Option<String>,

    pub(super) src: String,

    #[cfg_attr(not(feature = "offline"), allow(dead_code))]
    pub(super) src_span: Span,

    pub(super) record_type: RecordType,

    pub(super) arg_exprs: Vec<Expr>,

    pub(super) checked: bool,
}

enum QuerySrc {
    String(String),
    File(String),
}

pub enum RecordType {
    Given(Type),
    Generated,
}

impl Parse for QueryMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut query_src: Option<(QuerySrc, Span)> = None;
        let mut args: Option<Vec<Expr>> = None;
        let mut record_type = RecordType::Generated;
        let mut checked = true;
        let mut env_var = None;

        let mut expect_comma = false;

        while !input.is_empty() {
            if expect_comma {
                let _ = input.parse::<syn::token::Comma>()?;
            }

            let key: Ident = input.parse()?;

            let _ = input.parse::<syn::token::Eq>()?;

            if key == "env_var" {
                env_var = Some(input.parse::<LitStr>()?.value());
            } else if key == "source" {
                let span = input.span();
                let query_str = Punctuated::<LitStr, Token![+]>::parse_separated_nonempty(input)?
                    .iter()
                    .map(LitStr::value)
                    .collect();
                query_src = Some((QuerySrc::String(query_str), span));
            } else if key == "source_file" {
                let lit_str = input.parse::<LitStr>()?;
                query_src = Some((QuerySrc::File(lit_str.value()), lit_str.span()));
            } else if key == "args" {
                let exprs = input.parse::<ExprArray>()?;
                args = Some(exprs.elems.into_iter().collect())
            } else if key == "record" {
                record_type = RecordType::Given(input.parse()?);
            } else if key == "checked" {
                let lit_bool = input.parse::<LitBool>()?;
                checked = lit_bool.value;
            } else {
                let message = format!("unexpected input key: {}", key);
                return Err(syn::Error::new_spanned(key, message));
            }

            expect_comma = true;
        }

        let (src, src_span) =
            query_src.ok_or_else(|| input.error("expected `source` or `source_file` key"))?;

        let arg_exprs = args.unwrap_or_default();

        Ok(QueryMacroInput {
            env_var,
            src: src.resolve(src_span)?,
            src_span,
            record_type,
            arg_exprs,
            checked,
        })
    }
}

impl QuerySrc {
    /// If the query source is a file, read it to a string. Otherwise return the query string.
    fn resolve(self, source_span: Span) -> syn::Result<String> {
        match self {
            QuerySrc::String(string) => Ok(string),
            QuerySrc::File(file) => read_file_src(&file, source_span),
        }
    }
}

fn read_file_src(source: &str, source_span: Span) -> syn::Result<String> {
    let file_path = crate::common::resolve_path(source, source_span)?;

    fs::read_to_string(&file_path).map_err(|e| {
        syn::Error::new(
            source_span,
            format!(
                "failed to read query file at {}: {}",
                file_path.display(),
                e
            ),
        )
    })
}
