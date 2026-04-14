use std::fs;
use std::path::Path;

pub fn cmd_docker_gen(root: &str) {
    let root = Path::new(root);
    let app_name = detect_app_name(root).unwrap_or_else(|| "myapp".into());

    write_if_new(&root.join("docker-compose.yml"), &compose_yml(&app_name));
    write_if_new(&root.join("Dockerfile"),         &dockerfile(&app_name));
    write_if_new(&root.join(".dockerignore"),      DOCKERIGNORE);

    println!("✓  docker-compose.yml");
    println!("✓  Dockerfile");
    println!("✓  .dockerignore");
    println!();
    println!("Start services:");
    println!("  docker compose up -d db        # Postgres only");
    println!("  docker compose up --build      # Postgres + app");
    println!();
    println!("Connect to Postgres:");
    println!("  postgresql://postgres:postgres@localhost:5432/{}_dev", app_name);
}

fn detect_app_name(root: &Path) -> Option<String> {
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            if let Some(val) = rest.split('"').nth(1) {
                return Some(val.replace('-', "_"));
            }
        }
    }
    None
}

fn write_if_new(path: &Path, content: &str) {
    if path.exists() {
        println!("  (skipped — already exists) {}", path.display());
        return;
    }
    fs::write(path, content).unwrap_or_else(|e| eprintln!("error writing {}: {e}", path.display()));
}

fn compose_yml(app: &str) -> String {
    format!(r#"services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER:     postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB:       {app}_dev
    ports:
      - "5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout:  5s
      retries:  5

  app:
    build: .
    ports:
      - "4000:4000"
    environment:
      DATABASE_URL: postgresql://postgres:postgres@db:5432/{app}_dev
      RUST_LOG:     info
    depends_on:
      db:
        condition: service_healthy

volumes:
  pgdata:
"#)
}

fn dockerfile(app: &str) -> String {
    format!(r#"# ── Build ────────────────────────────────────────────────────────────────────
FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release -p {app}

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM alpine:3 AS runtime
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/{app} ./server
EXPOSE 4000
CMD ["./server"]
"#)
}

const DOCKERIGNORE: &str = r#"target/
.git/
*.md
.env
"#;
