use cssparser::*;
use crate::traits::{Parse, ToCss};
use crate::printer::Printer;
use super::{CssRuleList, MinifyContext};
use crate::rules::{ToCssWithContext, StyleContext};
use crate::error::{ParserError, MinifyError, PrinterError};

#[derive(Debug, PartialEq)]
pub struct SupportsRule {
  pub condition: SupportsCondition,
  pub rules: CssRuleList,
  pub loc: SourceLocation
}

impl SupportsRule {
  pub(crate) fn minify(&mut self, context: &mut MinifyContext, parent_is_unused: bool) -> Result<(), MinifyError> {
    self.rules.minify(context, parent_is_unused)
  }
}

impl ToCssWithContext for SupportsRule {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext>) -> Result<(), PrinterError> where W: std::fmt::Write {
    dest.add_mapping(self.loc);
    dest.write_str("@supports ")?;
    self.condition.to_css(dest)?;
    dest.whitespace()?;
    dest.write_char('{')?;
    dest.indent();
    dest.newline()?;
    self.rules.to_css_with_context(dest, context)?;
    dest.dedent();
    dest.newline()?;
    dest.write_char('}')
  }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SupportsCondition {
  Not(Box<SupportsCondition>),
  And(Vec<SupportsCondition>),
  Or(Vec<SupportsCondition>),
  Declaration(String),
  Selector(String),
  // FontTechnology()
  Parens(Box<SupportsCondition>),
  Unknown(String)
}

impl Parse for SupportsCondition {
  fn parse<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if input.try_parse(|input| input.expect_ident_matching("not")).is_ok() {
      let in_parens = Self::parse_in_parens(input)?;
      return Ok(SupportsCondition::Not(Box::new(in_parens)))
    }

    let in_parens = Self::parse_in_parens(input)?;
    let mut expected_type = None;
    let mut conditions = Vec::new();

    loop {
      let condition = input.try_parse(|input| {
        let location = input.current_source_location();
        let s = input.expect_ident()?;
        let found_type = match_ignore_ascii_case! { &s,
          "and" => 1,
          "or" => 2,
          _ => return Err(location.new_unexpected_token_error(
            cssparser::Token::Ident(s.clone())
          ))
        };

        if let Some(expected) = expected_type {
          if found_type != expected {
            return Err(location.new_unexpected_token_error(
              cssparser::Token::Ident(s.clone())
            ))
          }
        } else {
          expected_type = Some(found_type);
        }

        Self::parse_in_parens(input)
      });

      if let Ok(condition) = condition {
        if conditions.is_empty() {
          conditions.push(in_parens.clone())
        }
        conditions.push(condition)
      } else {
        break
      }
    }

    match expected_type {
      Some(1) => Ok(SupportsCondition::And(conditions)),
      Some(2) => Ok(SupportsCondition::Or(conditions)),
      _ => Ok(in_parens)
    }
  }
}

impl SupportsCondition {
  fn parse_in_parens<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    input.skip_whitespace();
    let location = input.current_source_location();
    let pos = input.position();
    match input.next()? {
      Token::Function(ref f) => {
        match_ignore_ascii_case! { &*f,
          "selector" => {
            let res = input.try_parse(|input| {
              input.parse_nested_block(|input| {
                let pos = input.position();
                input.expect_no_error_token()?;
                Ok(SupportsCondition::Selector(input.slice_from(pos).to_owned()))
              })
            });
            if res.is_ok() {
              return res
            }
          },
          _ => {}
        }
      }
      Token::ParenthesisBlock => {
        let res = input.try_parse(|input| {
          input.parse_nested_block(|input| {
            if let Ok(condition) = input.try_parse(SupportsCondition::parse) {
              return Ok(SupportsCondition::Parens(Box::new(condition)))
            }
      
            Self::parse_declaration(input)
          })
        });
        if res.is_ok() {
          return res
        }
      }
      t => return Err(location.new_unexpected_token_error(t.clone())),
    };

    input.parse_nested_block(|input| input.expect_no_error_token().map_err(|err| err.into()))?;
    Ok(SupportsCondition::Unknown(input.slice_from(pos).to_owned()))
  }

  pub fn parse_declaration<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let pos = input.position();
    input.expect_ident()?;
    input.expect_colon()?;
    input.expect_no_error_token()?;
    Ok(SupportsCondition::Declaration(input.slice_from(pos).to_owned()))
  }
}

impl ToCss for SupportsCondition {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    match self {
      SupportsCondition::Not(condition) => {
        dest.write_str("not ")?;
        condition.to_css(dest)
      }
      SupportsCondition::And(conditions) => {
        let mut first = true;
        for condition in conditions {
          if first {
            first = false;
          } else {
            dest.write_str(" and ")?;
          }
          condition.to_css(dest)?;
        }
        Ok(())
      }
      SupportsCondition::Or(conditions) => {
        let mut first = true;
        for condition in conditions {
          if first {
            first = false;
          } else {
            dest.write_str(" or ")?;
          }
          condition.to_css(dest)?;
        }
        Ok(())
      }
      SupportsCondition::Parens(condition) => {
        dest.write_char('(')?;
        condition.to_css(dest)?;
        dest.write_char(')')
      }
      SupportsCondition::Declaration(decl) => {
        dest.write_char('(')?;
        dest.write_str(&decl)?;
        dest.write_char(')')
      }
      SupportsCondition::Selector(sel) => {
        dest.write_str("selector(")?;
        dest.write_str(sel)?;
        dest.write_char(')')
      }
      SupportsCondition::Unknown(unknown) => {
        dest.write_str(&unknown)
      }
    }
  }
}
