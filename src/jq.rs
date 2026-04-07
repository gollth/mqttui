#![allow(unused_assignments)]
use std::{fmt::Display, fs::OpenOptions, io::Write, iter::empty, ops::Range, path::PathBuf};

use color_eyre::{Result, eyre::Context};
use enum_as_inner::EnumAsInner;
use indexmap::IndexSet;
use itertools::Itertools;
use jaq_core::{
    Compiler, Ctx, Filter, Native, RcIter, compile,
    load::{self, Arena, File, Loader},
};
use jaq_json::Val;
use serde_json::Value;
use tracing::{info, warn};

use crate::config::Config;

const INITIAL_PROMPT: &str = ".";
const TOPIC_HISTORY_DELIMITER: &str = ":|:";

#[derive(Debug, Default, Clone, EnumAsInner)]
pub enum Jaqqer {
    #[default]
    Dormant,
    Prompt {
        /// Current prompt typed by user so far (can be invalid)
        prompt: String,
        /// Previously (valid) applied filter
        previous: String,
        /// Horizontal position of the cursor where to type the next charater into `prompt`
        cursor: u16,
        /// Vertical index in the history. 0 is using `prompt`, any bigger value the closest
        /// matching prompt which was last active
        index: usize,
        /// List of errors which are wrong with `prompt`
        errors: Reports,
    },
    Active {
        /// Currently active and valid prompt
        prompt: String,
        /// Cached horizontal cursor position from last edit, used to restore on ESC
        cursor: u16,
        /// Cached errors to restore on ESC
        errors: Reports,
    },
}

impl Jaqqer {
    /// Put this JQ filter in edit mode, if it isn't already
    pub(crate) fn edit(self, history: &mut History) -> Self {
        history.stage(INITIAL_PROMPT);
        match self {
            Self::Dormant => Self::Prompt {
                prompt: INITIAL_PROMPT.into(),
                previous: INITIAL_PROMPT.into(),
                errors: Default::default(),
                cursor: 1,
                index: 0,
            },
            Self::Prompt {
                prompt,
                previous,
                cursor,
                errors,
                index: history,
            } => Self::Prompt {
                prompt,
                previous,
                cursor,
                errors,
                index: history,
            },
            Self::Active {
                prompt,
                cursor,
                errors,
            } => Self::Prompt {
                previous: prompt.clone(),
                prompt,
                cursor,
                errors,
                index: 0,
            },
        }
    }

    /// Put this JQ filter in active mode, if it isn't already
    pub(crate) fn activate(self, history: &mut History, topic: &str) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt {
                prompt,
                cursor,
                errors,
                ..
            } => {
                info!(jq = prompt, "activate");
                history.commit(topic);
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

    pub(crate) fn input(mut self, c: char, history: &mut History) -> Self {
        if let Some((prompt, _, cursor, index, ..)) = self.as_prompt_mut() {
            prompt.insert(*cursor as usize, c);
            history.stage(prompt);
            *index = 0;
            *cursor += 1;
        }
        self.update_errors()
    }

    fn update_errors(mut self) -> Self {
        let e = self.compile(false).err().into_iter().flatten();
        if let Some((.., errors)) = self.as_prompt_mut() {
            errors.clear();
            errors.extend(e);
        }
        self
    }

    pub(crate) fn up(self, history: &History, topic: &str) -> Self {
        match self.into_prompt() {
            Err(original) => original,
            Ok((prompt, previous, cursor, index, errors)) => {
                let new_index = (index + 1).min(history.len(topic));
                history
                    .lookup(new_index, topic)
                    .map(|prompt| {
                        Self::Prompt {
                            cursor: prompt.chars().count() as u16,
                            prompt,
                            previous: previous.clone(),
                            index: new_index,
                            errors: Default::default(),
                        }
                        .update_errors()
                    })
                    .unwrap_or(Self::Prompt {
                        prompt,
                        previous,
                        cursor,
                        index,
                        errors,
                    })
            }
        }
    }

    pub(crate) fn down(self, history: &History, topic: &str) -> Self {
        match self.into_prompt() {
            Err(original) => original,
            Ok((prompt, previous, cursor, index, errors)) => {
                let new_index = index.saturating_sub(1).min(history.len(topic));
                history
                    .lookup(new_index, topic)
                    .map(|prompt| {
                        Self::Prompt {
                            cursor: prompt.chars().count() as u16,
                            prompt,
                            previous: previous.clone(),
                            index: new_index,
                            errors: Default::default(),
                        }
                        .update_errors()
                    })
                    .unwrap_or(Self::Prompt {
                        prompt,
                        previous,
                        cursor,
                        index,
                        errors,
                    })
            }
        }
    }

    pub(crate) fn backspace(mut self, history: &mut History) -> Self {
        if let Some((prompt, _, cursor, index, ..)) = self.as_prompt_mut()
            && !prompt.is_empty()
            && *cursor > 0
        {
            prompt.remove(*cursor as usize - 1);
            history.stage(prompt);
            *index = 0;

            *cursor -= 1;
        }
        let e = self.compile(false).err().into_iter().flatten();
        if let Some((.., errors)) = self.as_prompt_mut() {
            errors.clear();
            errors.extend(e);
        }
        self
    }

    pub(crate) fn delete(mut self, history: &mut History) -> Self {
        if let Some((prompt, _, cursor, index, ..)) = self.as_prompt_mut() {
            let c = *cursor as usize;
            if !prompt.is_empty() && c < prompt.chars().count() {
                prompt.remove(c);
                history.stage(prompt);
                *index = 0;
            }
        }
        let e = self.compile(false).err().into_iter().flatten();
        if let Some((.., errors)) = self.as_prompt_mut() {
            errors.clear();
            errors.extend(e);
        }
        self
    }

    pub(crate) fn compile(&self, last_active: bool) -> Result<Filter<Native<Val>>, Reports> {
        let code = match self {
            Self::Dormant => return Ok(Default::default()),
            Self::Prompt { previous, .. } if last_active => previous.as_str(),
            Self::Prompt { prompt, .. } => prompt.as_str(),
            Self::Active { prompt, .. } => prompt.as_str(),
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
        let filter = self.compile(true)?;
        let input = RcIter::new(empty());
        Ok(filter
            .run((Ctx::new([], &input), Val::from(value)))
            .filter_map(|value| value.ok())
            .map(Value::from)
            .collect())
    }
}

pub struct History {
    path: PathBuf,
    commited: IndexSet<Item>,
    staging: String,
}

/// A history item composed of the JQ filter and the topic on which it was used
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct Item {
    topic: String,
    filter: String,
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{TOPIC_HISTORY_DELIMITER}{}", self.topic, self.filter)
    }
}

impl Item {
    fn new(topic: &str, filter: &str) -> Self {
        Self {
            topic: topic.split('/').next_back().unwrap_or(topic).into(),
            filter: filter.into(),
        }
    }
    fn matches(&self, topic: &str) -> bool {
        let suffix = topic.split('/').next_back().unwrap_or(topic);
        self.topic.is_empty() || self.topic.ends_with(suffix)
    }
}

impl History {
    pub fn load() -> Result<Self> {
        let path = Config::history()?;
        let content = std::fs::read_to_string(&path).wrap_err(path.display().to_string())?;
        let list = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (topic, filter) = line
                    .split(TOPIC_HISTORY_DELIMITER)
                    .next_tuple()
                    .unwrap_or(("", line));
                Item::new(topic, filter)
            })
            .rev()
            .collect::<IndexSet<_>>();

        tracing::debug!(amount = list.len(), "loading JQ history");
        Ok(Self {
            path,
            commited: list,
            staging: Default::default(),
        })
    }

    fn stage(&mut self, prompt: &str) {
        tracing::debug!(prompt = prompt, "stage");
        self.staging = prompt.into();
    }

    fn commit(&mut self, topic: &str) {
        if self.staging.is_empty() || self.staging == INITIAL_PROMPT {
            return;
        }
        let commit = self.staging.clone();
        let topic = topic.split('/').next_back().unwrap_or(topic);
        info!(commit = commit, topic = topic, "History::commit");
        let item = Item::new(topic, &commit);
        let new_value = self.commited.shift_insert(0, item.clone());
        if !new_value {
            return;
        }

        let result = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .wrap_err("failed to open history file")
            .and_then(|mut file| {
                writeln!(file, "{item}").wrap_err("failed to append prompt to history file")
            });
        if let Err(e) = result {
            warn!(prompt = commit, "{e}");
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Item> {
        self.commited
            .iter()
            .filter(|item| item.filter.starts_with(&self.staging))
    }

    fn matching(&self, topic: &str) -> impl Iterator<Item = &Item> {
        self.iter().filter(|item| item.matches(topic))
    }

    fn is_empty(&self, topic: &str) -> bool {
        self.len(topic) == 0
    }

    fn len(&self, topic: &str) -> usize {
        self.matching(topic).count()
    }

    /// Lookup from bottom of history
    fn lookup(&self, index: usize, topic: &str) -> Option<String> {
        if self.is_empty(topic) {
            return None;
        }
        if index == 0 {
            info!(index = index, prompt = self.staging, "History::lookup");
            return Some(self.staging.clone());
        }

        let commit = self.matching(topic).nth(index - 1)?.filter.as_str();
        info!(
            index = index,
            topic = topic,
            prompt = commit,
            "History::lookup"
        );
        Some(commit.into())
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
            message: format!("could not load file {path}: {error}"),
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
