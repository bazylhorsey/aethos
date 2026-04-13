use std::fmt;

/// The output type of every `h!{}` template and function component.
#[derive(Clone, Default)]
pub struct Html(pub String);

impl Html {
    pub fn new(s: impl Into<String>) -> Self {
        Html(s.into())
    }

    pub fn empty() -> Self {
        Html(String::new())
    }

    /// Used internally by the `h!` macro.
    pub fn from_tokens(f: impl FnOnce() -> String) -> Self {
        Html(f())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Alias for the inner string — allows component functions returning `Html`
    /// to be called from `h!`-generated code which calls `.render_string()`.
    pub fn render_string(&self) -> String {
        self.0.clone()
    }

    /// Concatenate another `Html` fragment.
    pub fn append(mut self, other: Html) -> Self {
        self.0.push_str(&other.0);
        self
    }
}

impl fmt::Display for Html {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Html {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Html({:?})", self.0)
    }
}

/// Trusted HTML — bypasses escaping. Only use with values you control.
#[derive(Clone, Debug)]
pub struct Safe(pub String);

impl Safe {
    pub fn new(s: impl Into<String>) -> Self {
        Safe(s.into())
    }
}

impl From<Safe> for Html {
    fn from(s: Safe) -> Self {
        Html(s.0)
    }
}


