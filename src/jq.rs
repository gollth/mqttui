use std::{iter::empty, ops::Range};

use color_eyre::Result;
use enum_as_inner::EnumAsInner;
use jaq_core::{
    Compiler, Ctx, Filter, Native, RcIter, compile,
    load::{self, Arena, File, Loader},
};
use jaq_json::Val;
use serde_json::Value;
use tracing::info;

#[derive(Debug, Default, Clone, EnumAsInner)]
pub enum Jaqqer {
    #[default]
    Dormant,
    Prompt {
        prompt: String,
        cursor: u16,
        errors: Reports,
    },
    Active {
        prompt: String,
        cursor: u16,
        errors: Reports,
    },
}

impl Jaqqer {
    /// Put this JQ filter in edit mode, if it isn't already
    pub(crate) fn edit(self) -> Self {
        match self {
            Self::Dormant => Self::Prompt {
                prompt: ".".into(),
                errors: Default::default(),
                cursor: 1,
            },
            Self::Prompt {
                prompt,
                cursor,
                errors,
            } => Self::Prompt {
                prompt,
                cursor,
                errors,
            },
            Self::Active {
                prompt,
                cursor,
                errors,
            } => Self::Prompt {
                prompt,
                cursor,
                errors,
            },
        }
    }

    /// Put this JQ filter in active mode, if it isn't already
    pub(crate) fn activate(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt {
                prompt,
                cursor,
                errors,
            } => {
                info!(jq = prompt, "activate");
                Self::Active {
                    prompt,
                    cursor,
                    errors,
                }
            }
            Self::Active {
                prompt,
                cursor,
                errors,
            } => Self::Active {
                prompt,
                cursor,
                errors,
            },
        }
    }

    pub(crate) fn errors(&self) -> &[Report] {
        match self {
            Self::Dormant => &[],
            Self::Prompt { errors, .. } => errors,
            Self::Active { errors, .. } => errors,
        }
    }

    /// Rest this JQ filter and put it back in dormant mode
    pub(crate) fn clear(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt { .. } => Self::Dormant,
            Self::Active { .. } => Self::Dormant,
        }
    }

    pub(crate) fn move_cursor(mut self, offset: i16) -> Jaqqer {
        if let Self::Prompt { prompt, cursor, .. } = &mut self {
            *cursor = ((*cursor as i16) + offset).clamp(0, prompt.chars().count() as i16) as u16;
        }
        self
    }

    pub(crate) fn input(mut self, c: char) -> Self {
        if let Some((prompt, cursor, ..)) = self.as_prompt_mut() {
            prompt.insert(*cursor as usize, c);
            *cursor += 1;
        }
        let e = self.compile().err().into_iter().flatten();
        if let Some((.., errors)) = self.as_prompt_mut() {
            errors.clear();
            errors.extend(e);
        }
        self
    }

    pub(crate) fn backspace(mut self) -> Self {
        if let Some((prompt, cursor, ..)) = self.as_prompt_mut() {
            if !prompt.is_empty() && *cursor > 0 {
                prompt.remove(*cursor as usize - 1);
                *cursor -= 1;
            }
        }
        let e = self.compile().err().into_iter().flatten();
        if let Some((.., errors)) = self.as_prompt_mut() {
            errors.clear();
            errors.extend(e);
        }
        self
    }

    pub(crate) fn delete(mut self) -> Self {
        if let Some((prompt, cursor, ..)) = self.as_prompt_mut() {
            let c = *cursor as usize;
            if !prompt.is_empty() && c < prompt.chars().count() {
                prompt.remove(c);
            }
        }
        self
    }

    pub(crate) fn compile(&self) -> Result<Filter<Native<Val>>, Reports> {
        let Some(code) = self
            .as_prompt()
            .or(self.as_active())
            .map(|(p, ..)| p.as_str())
        else {
            return Ok(Default::default());
        };
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let arena = Arena::default();

        let program = File { code, path: () };
        let modules = loader.load(&arena, program).map_err(|e| {
            e.into_iter()
                .flat_map(|(file, err)| match err {
                    jaq_core::load::Error::Io(e) => e
                        .into_iter()
                        .map(|e| Report::io(file.code, e))
                        .collect::<Vec<_>>(),
                    jaq_core::load::Error::Lex(e) => e
                        .into_iter()
                        .map(|e| Report::lexer(file.code, e))
                        .collect::<Vec<_>>(),
                    jaq_core::load::Error::Parse(e) => e
                        .into_iter()
                        .map(|e| Report::parse(file.code, e))
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>()
        })?;

        let filter = Compiler::default()
            .with_funs(jaq_std::funs().chain(jaq_json::funs()))
            .compile(modules)
            .map_err(|e| {
                e.into_iter()
                    .flat_map(|(file, err)| err.into_iter().map(|e| Report::compile(file.code, e)))
                    .collect::<Vec<_>>()
            })?;
        Ok(filter)
    }

    pub(crate) fn run(&self, value: Value) -> Result<Vec<Value>, Reports> {
        let filter = self.compile()?;
        let input = RcIter::new(empty());
        Ok(filter
            .run((Ctx::new([], &input), Val::from(value)))
            .filter_map(|value| value.ok())
            .map(Value::from)
            .collect())
    }
}

pub type Reports = Vec<Report>;

#[derive(Debug, Clone)]
pub struct Report {
    pub message: String,
    pub span: Range<usize>,
}

impl Report {
    pub(crate) fn compile(code: &str, (found, undefined): compile::Error<&str>) -> Self {
        use compile::Undefined::Filter;
        let wnoa = |exp, got| format!("wrong number of arguments (expected {exp}, found {got})");
        Self {
            message: match (found, undefined) {
                ("reduce", Filter(arity)) => wnoa("2", arity),
                ("foreach", Filter(arity)) => wnoa("2 or 3", arity),
                (_, undefined) => format!("undefined {}", undefined.as_str()),
            },
            span: load::span(code, found),
        }
    }

    fn io(code: &str, (path, error): (&str, String)) -> Self {
        Report {
            message: format!("could not load file {}: {}", path, error),
            span: load::span(code, path),
        }
    }

    fn lexer(code: &str, (expected, found): load::lex::Error<&str>) -> Self {
        let span = load::span(code, found);
        let unexpected = match &found[..found.char_indices().nth(1).map_or(found.len(), |(i, _)| i)]
        {
            "" => "end of input",
            _ => "character",
        };

        let expected = match &expected {
            load::lex::Expect::Delim(_) => "unclosed delimiter",
            _ => expected.as_str(),
        };

        Report {
            message: format!("unexpected {unexpected}, expected {expected}"),
            span,
        }
    }

    fn parse(code: &str, (expected, found): load::parse::Error<&str>) -> Self {
        let span = load::span(code, found);

        let unexpected = if found.is_empty() {
            "end of input"
        } else {
            "token"
        };
        Report {
            message: format!("unexpected {unexpected}, expected {}", expected.as_str()),
            span,
        }
    }

    pub(crate) fn display(&self) -> String {
        // TODO: Use codesnake here
        format!("{} ({:?})", self.message, self.span)
    }
}
