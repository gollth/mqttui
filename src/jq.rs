use std::iter::empty;

use color_eyre::{Result, eyre::eyre};
use enum_as_inner::EnumAsInner;
use jaq_core::{
    Compiler, Ctx, RcIter,
    load::{Arena, File, Loader},
};
use jaq_json::Val;
use serde_json::Value;

#[derive(Debug, Default, Clone, EnumAsInner)]
pub enum Jaqqer {
    #[default]
    Dormant,
    Prompt {
        prompt: String,
        cursor: u16,
    },
    Active(String),
}

impl Jaqqer {
    /// Put this JQ filter in edit mode, if it isn't already
    pub(crate) fn edit(self) -> Self {
        match self {
            Self::Dormant => Self::Prompt {
                prompt: String::new(),
                cursor: 0,
            },
            Self::Prompt { prompt, cursor } => Self::Prompt { prompt, cursor },
            Self::Active(prompt) => Self::Prompt { prompt, cursor: 0 },
        }
    }

    /// Put this JQ filter in active mode, if it isn't already
    pub(crate) fn activate(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt { prompt, .. } => Self::Active(prompt),
            Self::Active(p) => Self::Active(p),
        }
    }

    /// Rest this JQ filter and put it back in dormant mode
    pub(crate) fn clear(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt { .. } => Self::Dormant,
            Self::Active(_) => Self::Dormant,
        }
    }

    pub(crate) fn move_cursor(mut self, offset: i16) -> Jaqqer {
        if let Self::Prompt { prompt, cursor, .. } = &mut self {
            *cursor = ((*cursor as i16) + offset).clamp(0, prompt.chars().count() as i16) as u16;
        }
        self
    }

    pub(crate) fn input(mut self, c: char) -> Self {
        if let Some((prompt, cursor)) = self.as_prompt_mut() {
            prompt.push(c);
            *cursor += 1;
        }
        self
    }

    pub(crate) fn backspace(mut self) -> Self {
        if let Some((prompt, cursor)) = self.as_prompt_mut() {
            if !prompt.is_empty() && *cursor > 0 {
                prompt.remove(*cursor as usize - 1);
                *cursor -= 1;
            }
        }
        self
    }

    pub(crate) fn delete(mut self) -> Self {
        if let Some((prompt, cursor)) = self.as_prompt_mut() {
            let c = *cursor as usize;
            if !prompt.is_empty() && c < prompt.chars().count() {
                prompt.remove(c);
            }
        }
        self
    }

    pub(crate) fn run(&self, value: Value) -> Result<Vec<Value>> {
        let Some(code) = self
            .as_prompt()
            .map(|(p, _)| p)
            .or(self.as_active())
            .map(|p| p.as_str())
        else {
            return Ok(Vec::new());
        };
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let arena = Arena::default();

        let program = File { code, path: () };
        let modules = loader.load(&arena, program).map_err(|e| eyre!("{e:?}"))?;

        let filter = Compiler::default()
            .with_funs(jaq_std::funs().chain(jaq_json::funs()))
            .compile(modules)
            .map_err(|e| eyre!("{e:?}"))?;
        let input = RcIter::new(empty());
        Ok(filter
            .run((Ctx::new([], &input), Val::from(value)))
            .filter_map(|value| value.ok())
            .map(Value::from)
            .collect())
    }
}
