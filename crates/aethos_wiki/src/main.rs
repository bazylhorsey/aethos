mod state;
mod models;
mod templates;
mod controllers;

use aethos::{router, Logger, BodyParser, SecureHeaders};
use aethos_orm::{Repo, MigrationRunner};
use axum::Extension;
use std::sync::Arc;

use state::AppState;
use controllers::PageController;

fn build_router(state: AppState) -> axum::Router {
    router! {
        pipeline :browser {
            plug!(Logger);
            plug!(BodyParser);
            plug!(SecureHeaders);
        }

        scope "/" {
            pipe_through!(:browser);
            get!("/", PageController, index);
            get!("/wiki/new", PageController, new_form);
            post!("/wiki/new", PageController, create);
            get!("/wiki/:title/edit", PageController, edit_form);
            post!("/wiki/:title/edit", PageController, update);
            get!("/wiki/:title", PageController, show);
            get!("/search", PageController, search);
            get!("/random", PageController, random);
        }
    }
    .layer(Extension(state))
}

async fn seed_if_empty(repo: &Repo<sqlx::Sqlite>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::models::Entry;

    if Entry::count(repo).await > 0 {
        return Ok(());
    }

    let seeds: &[(&str, &str)] = &[
        (
            "HTML",
            r#"# HTML

**HTML** (HyperText Markup Language) is the standard markup language for creating web pages.

## Overview

HTML describes the structure of a web page using *elements* represented by tags. Browsers read
HTML documents and render them into visible web pages.

## Key Concepts

- **Elements** are the building blocks of HTML pages
- **Tags** like `<p>`, `<h1>`, and `<div>` define the structure
- **Attributes** provide additional information about elements
- **Links** use `<a href="...">` to connect pages together

## Example

A simple HTML document looks like:

```html
<!DOCTYPE html>
<html>
  <head><title>My Page</title></head>
  <body><h1>Hello World</h1></body>
</html>
```

## See Also

- [CSS](/wiki/CSS)
- [Git](/wiki/Git)
"#,
        ),
        (
            "CSS",
            r#"# CSS

**CSS** (Cascading Style Sheets) is the language used to style and layout HTML documents.

## Overview

CSS describes how HTML elements should be displayed on screen, on paper, or in other media.
CSS saves a lot of work by controlling the layout of multiple web pages at once.

## Key Concepts

- **Selectors** target HTML elements to style
- **Properties** define what aspect to change (color, size, spacing)
- **Values** specify how to change the property
- **Cascade** determines which styles apply when multiple rules match

## Example

```css
body {
  font-family: Arial, sans-serif;
  color: #333;
}

h1 {
  color: navy;
  font-size: 2em;
}
```

## See Also

- [HTML](/wiki/HTML)
"#,
        ),
        (
            "Git",
            r#"# Git

**Git** is a free and open source distributed version control system designed to handle projects
of all sizes with speed and efficiency.

## Overview

Git tracks changes in any set of files, usually used for coordinating work among programmers
collaboratively developing source code during software development.

## Key Commands

- `git init` — initialize a new repository
- `git clone <url>` — clone a remote repository
- `git add <file>` — stage changes for commit
- `git commit -m "message"` — record staged changes
- `git push` — upload changes to remote
- `git pull` — fetch and merge remote changes
- `git branch` — list, create, or delete branches
- `git merge` — join two development histories

## Workflow

1. Make changes to files
2. Stage the changes with `git add`
3. Commit with a descriptive message
4. Push to share with others

## See Also

- [Rust](/wiki/Rust)
"#,
        ),
        (
            "Rust",
            r#"# Rust

**Rust** is a multi-paradigm, general-purpose programming language that emphasizes performance,
type safety, and concurrency.

## Overview

Rust enforces memory safety, meaning that all references point to valid memory. It achieves
memory safety without a garbage collector by using a system of *ownership* with a set of rules
the compiler checks.

## Key Features

- **Ownership** system prevents dangling pointers and data races
- **Borrowing** allows references without transferring ownership
- **Lifetimes** ensure references are always valid
- **Zero-cost abstractions** — abstractions compile away
- **Cargo** — built-in package manager and build tool

## Hello World

```rust
fn main() {
    println!("Hello, world!");
}
```

## See Also

- [Git](/wiki/Git)
- [Python](/wiki/Python)
"#,
        ),
        (
            "Python",
            r#"# Python

**Python** is a high-level, general-purpose programming language known for its clear syntax
and readability.

## Overview

Python supports multiple programming paradigms, including structured, object-oriented, and
functional programming. Its comprehensive standard library gives it the nickname
*"batteries included"*.

## Key Features

- **Readable syntax** — uses indentation instead of braces
- **Dynamic typing** — no need to declare variable types
- **Interpreted** — runs code directly without compilation
- **Large ecosystem** — thousands of packages via pip

## Hello World

```python
print("Hello, world!")
```

## Common Uses

- Web development (Django, Flask)
- Data science and machine learning (NumPy, pandas)
- Scripting and automation
- Scientific computing

## See Also

- [Rust](/wiki/Rust)
"#,
        ),
    ];

    for (title, content) in seeds {
        Entry::create(repo, title, content).await?;
        tracing::info!(title = %title, "seeded entry");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let repo = Repo::<sqlx::Sqlite>::connect("sqlite://wiki.db?mode=rwc").await?;

    let runner = MigrationRunner::new("migrations");
    runner.run(repo.pool()).await?;

    seed_if_empty(&repo).await?;

    let state = AppState { repo: Arc::new(repo) };
    let router = build_router(state);

    let addr = "127.0.0.1:4000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Wiki running at http://{}", addr);
    axum::serve(listener, router).await?;

    Ok(())
}
