//! `Template` — the output of every `h!` template.
//!
//! A `Template` separates **static** HTML fragments (compile-time constants)
//! from **dynamic** expression values (computed at runtime). On subsequent
//! renders only the dynamic slots that changed need to be transmitted — the
//! statics are already known by the client.
//!
//! # Wire format (Phoenix-compatible)
//!
//! **Initial render:**
//! ```json
//! { "s": ["<div><p>", "</p><span>", "</span></div>"],
//!   "0": "Alice",
//!   "1": "42" }
//! ```
//!
//! **Diff (only changed slots):**
//! ```json
//! { "1": "43" }
//! ```

/// A compiled `h!` template.
///
/// Statics are `'static` string slices (stored in the binary's `.rodata`).
/// Dynamics are runtime-computed strings, one per dynamic expression in the
/// template. Statics and dynamics are interleaved: the rendered HTML is
/// `statics[0] + dynamics[0] + statics[1] + dynamics[1] + … + statics[n]`.
#[derive(Clone, Debug, Default)]
pub struct Template {
    /// Compile-time static string fragments (len == dynamics.len() + 1).
    pub statics: Vec<&'static str>,
    /// Runtime-evaluated dynamic slot values.
    pub dynamics: Vec<String>,
}

impl Template {
    /// Render to a complete HTML string by interleaving statics and dynamics.
    pub fn render_string(&self) -> String {
        let cap = self.statics.iter().map(|s| s.len()).sum::<usize>()
            + self.dynamics.iter().map(|d| d.len()).sum::<usize>();
        let mut out = String::with_capacity(cap);
        for (i, s) in self.statics.iter().enumerate() {
            out.push_str(s);
            if let Some(d) = self.dynamics.get(i) {
                out.push_str(d);
            }
        }
        out
    }

    /// Render to an `Html` value.
    pub fn render(&self) -> crate::Html {
        crate::Html(self.render_string())
    }

    /// Compute the diff against a previous render.
    ///
    /// Returns `Some(Vec<(index, &new_value)>)` containing only slots that
    /// changed. Returns `None` when the template has a different number of
    /// dynamic slots (structural change), signalling the transport to send a
    /// full re-render instead.
    pub fn diff_pairs<'a>(&'a self, prev: &Template) -> Option<Vec<(usize, &'a str)>> {
        if self.dynamics.len() != prev.dynamics.len() {
            return None;
        }
        let pairs = self
            .dynamics
            .iter()
            .zip(prev.dynamics.iter())
            .enumerate()
            .filter(|(_, (next, prev))| next != prev)
            .map(|(i, (next, _))| (i, next.as_str()))
            .collect();
        Some(pairs)
    }

    /// Returns `true` if every dynamic slot is identical to the previous render.
    pub fn is_same_as(&self, prev: &Template) -> bool {
        self.dynamics.len() == prev.dynamics.len()
            && self.dynamics.iter().zip(prev.dynamics.iter()).all(|(a, b)| a == b)
    }
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render_string())
    }
}

impl From<Template> for crate::Html {
    fn from(t: Template) -> Self {
        crate::Html(t.render_string())
    }
}

/// Convert a raw `Html` value into a single-slot `Template` (for compatibility).
impl From<crate::Html> for Template {
    fn from(h: crate::Html) -> Self {
        Template {
            statics: vec!["", ""],
            dynamics: vec![h.0],
        }
    }
}
