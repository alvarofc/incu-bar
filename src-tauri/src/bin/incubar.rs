use incubar_tauri_lib::providers::{
    load_cost_snapshot, ProviderId, ProviderRegistry, StatusIndicator,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug)]
struct CliArgs {
    command: String,
    format: OutputFormat,
    provider: Option<String>,
    pretty: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    provider: String,
    indicator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CostPayload {
    provider: String,
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_tokens: Option<u64>,
    #[serde(rename = "sessionCostUSD", skip_serializing_if = "Option::is_none")]
    session_cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_30_days_tokens: Option<u64>,
    #[serde(rename = "last30DaysCostUSD", skip_serializing_if = "Option::is_none")]
    last_30_days_cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = parse_args(std::env::args().skip(1).collect());
    if args.command == "--help" || args.command == "-h" {
        print_help();
        return;
    }
    if args.command == "--version" || args.command == "-V" {
        println!("incubar {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let result = match args.command.as_str() {
        "status" => run_status(args).await,
        "cost" => run_cost(args).await,
        "usage" => Err("usage is not supported in the bundled CLI".to_string()),
        _ => Err(format!(
            "Unknown command: {}. Use --help for usage.",
            args.command
        )),
    };

    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn parse_args(mut argv: Vec<String>) -> CliArgs {
    let mut format = OutputFormat::Text;
    let mut pretty = false;
    let mut provider = None;
    let mut command = String::new();
    let mut json_output = false;

    if let Some(first) = argv.first() {
        if !first.starts_with('-') {
            command = argv.remove(0);
        }
    }

    if command.is_empty() {
        command = "status".to_string();
    }

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--format" => {
                if let Some(value) = iter.next() {
                    if value == "json" {
                        format = OutputFormat::Json;
                    } else if value == "text" {
                        format = OutputFormat::Text;
                    }
                }
            }
            "--json" => {
                format = OutputFormat::Json;
                json_output = true;
            }
            "--json-output" => {
                format = OutputFormat::Json;
                json_output = true;
            }
            "--pretty" => pretty = true,
            "--provider" => provider = iter.next(),
            "--help" | "-h" | "--version" | "-V" => {
                command = arg;
                break;
            }
            _ => {}
        }
    }

    CliArgs {
        command,
        format,
        provider,
        pretty: pretty || json_output,
    }
}

async fn run_status(args: CliArgs) -> Result<(), String> {
    let providers = select_providers(args.provider.as_deref(), ProviderSelectionKind::All)?;
    let registry = ProviderRegistry::new();
    let mut payloads = Vec::new();
    let mut sections = Vec::new();

    for provider_id in providers {
        let status = registry
            .fetch_status(&provider_id)
            .await
            .map_err(|err| err.to_string())?;

        let provider_name = provider_id_string(provider_id);
        let url = status_page_url(provider_id);
        let payload = StatusPayload {
            provider: provider_name.to_string(),
            indicator: status_indicator_string(status.indicator).to_string(),
            description: status.description.clone(),
            updated_at: status.updated_at.clone(),
            url,
        };

        match args.format {
            OutputFormat::Text => sections.push(render_status_text(provider_name, &payload)),
            OutputFormat::Json => payloads.push(payload),
        }
    }

    match args.format {
        OutputFormat::Text => {
            if !sections.is_empty() {
                println!("{}", sections.join("\n\n"));
            }
        }
        OutputFormat::Json => print_json(&payloads, args.pretty)?,
    }

    Ok(())
}

async fn run_cost(args: CliArgs) -> Result<(), String> {
    let providers = select_providers(args.provider.as_deref(), ProviderSelectionKind::CostOnly)?;
    if providers.is_empty() {
        return Err("cost is only supported for codex and claude".to_string());
    }
    let mut payloads = Vec::new();
    let mut sections = Vec::new();
    for provider_id in providers {
        let provider_name = provider_id_string(provider_id).to_string();
        let snapshot = load_cost_snapshot(provider_id).await;
        let payload = match snapshot {
            Some(snapshot) => CostPayload {
                provider: provider_name.clone(),
                source: "local",
                updated_at: Some(chrono::Utc::now().to_rfc3339()),
                session_tokens: Some(snapshot.today_tokens),
                session_cost_usd: Some(snapshot.today_amount),
                last_30_days_tokens: Some(snapshot.month_tokens),
                last_30_days_cost_usd: Some(snapshot.month_amount),
                error: None,
            },
            None => CostPayload {
                provider: provider_name.clone(),
                source: "local",
                updated_at: None,
                session_tokens: None,
                session_cost_usd: None,
                last_30_days_tokens: None,
                last_30_days_cost_usd: None,
                error: None,
            },
        };

        match args.format {
            OutputFormat::Text => sections.push(render_cost_text(&provider_name, &payload)),
            OutputFormat::Json => payloads.push(payload),
        }
    }

    match args.format {
        OutputFormat::Text => {
            if !sections.is_empty() {
                println!("{}", sections.join("\n\n"));
            }
        }
        OutputFormat::Json => print_json(&payloads, args.pretty)?,
    }

    Ok(())
}

fn render_status_text(provider: &str, payload: &StatusPayload) -> String {
    let mut lines = Vec::new();
    lines.push(format!("== {provider} Status =="));
    let mut status_line = format!("Status: {}", status_label(payload.indicator.as_str()));
    if let Some(description) = payload
        .description
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        status_line.push_str(" - ");
        status_line.push_str(description);
    }
    lines.push(status_line);
    if let Some(updated_at) = payload.updated_at.as_ref() {
        lines.push(format!("Updated: {updated_at}"));
    }
    if let Some(url) = payload.url.as_ref() {
        lines.push(format!("URL: {url}"));
    }
    lines.join("\n")
}

fn render_cost_text(provider: &str, payload: &CostPayload) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{provider} Cost (local)"));
    let today_cost = payload
        .session_cost_usd
        .map(format_usd)
        .unwrap_or_else(|| "—".to_string());
    let today_tokens = payload
        .session_tokens
        .map(format_tokens)
        .map(|tokens| format!(" · {tokens} tokens"))
        .unwrap_or_default();
    lines.push(format!("Today: {today_cost}{today_tokens}"));

    let month_cost = payload
        .last_30_days_cost_usd
        .map(format_usd)
        .unwrap_or_else(|| "—".to_string());
    let month_tokens = payload
        .last_30_days_tokens
        .map(format_tokens)
        .map(|tokens| format!(" · {tokens} tokens"))
        .unwrap_or_default();
    lines.push(format!("Last 30 days: {month_cost}{month_tokens}"));
    lines.join("\n")
}

fn format_usd(amount: f64) -> String {
    format!("${:.2}", amount)
}

fn format_tokens(tokens: u64) -> String {
    let text = tokens.to_string();
    let mut out = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn print_json<T: Serialize>(payload: &T, pretty: bool) -> Result<(), String> {
    let output = if pretty {
        serde_json::to_string_pretty(payload)
    } else {
        serde_json::to_string(payload)
    }
    .map_err(|err| err.to_string())?;
    println!("{output}");
    Ok(())
}

#[derive(Clone, Copy)]
enum ProviderSelectionKind {
    All,
    CostOnly,
}

fn select_providers(
    provider: Option<&str>,
    kind: ProviderSelectionKind,
) -> Result<Vec<ProviderId>, String> {
    let all = ProviderId::all();
    let selected = match provider {
        Some("all") | None => all,
        Some(value) => vec![parse_provider(value)?],
    };

    Ok(match kind {
        ProviderSelectionKind::All => selected,
        ProviderSelectionKind::CostOnly => selected
            .into_iter()
            .filter(|id| matches!(id, ProviderId::Codex | ProviderId::Claude))
            .collect(),
    })
}

fn parse_provider(value: &str) -> Result<ProviderId, String> {
    match value {
        "claude" => Ok(ProviderId::Claude),
        "codex" => Ok(ProviderId::Codex),
        "cursor" => Ok(ProviderId::Cursor),
        "copilot" => Ok(ProviderId::Copilot),
        "gemini" => Ok(ProviderId::Gemini),
        "antigravity" => Ok(ProviderId::Antigravity),
        "factory" => Ok(ProviderId::Factory),
        "zai" => Ok(ProviderId::Zai),
        "minimax" => Ok(ProviderId::Minimax),
        "kimi" => Ok(ProviderId::Kimi),
        "kimi_k2" => Ok(ProviderId::KimiK2),
        "kiro" => Ok(ProviderId::Kiro),
        "vertexai" => Ok(ProviderId::Vertex),
        "augment" => Ok(ProviderId::Augment),
        "amp" => Ok(ProviderId::Amp),
        "jetbrains" => Ok(ProviderId::Jetbrains),
        "opencode" => Ok(ProviderId::Opencode),
        "synthetic" => Ok(ProviderId::Synthetic),
        _ => Err(format!("Unknown provider: {value}")),
    }
}

fn provider_id_string(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Claude => "claude",
        ProviderId::Codex => "codex",
        ProviderId::Cursor => "cursor",
        ProviderId::Copilot => "copilot",
        ProviderId::Gemini => "gemini",
        ProviderId::Antigravity => "antigravity",
        ProviderId::Factory => "factory",
        ProviderId::Zai => "zai",
        ProviderId::Minimax => "minimax",
        ProviderId::Kimi => "kimi",
        ProviderId::KimiK2 => "kimi_k2",
        ProviderId::Kiro => "kiro",
        ProviderId::Vertex => "vertexai",
        ProviderId::Augment => "augment",
        ProviderId::Amp => "amp",
        ProviderId::Jetbrains => "jetbrains",
        ProviderId::Opencode => "opencode",
        ProviderId::Synthetic => "synthetic",
    }
}

fn status_indicator_string(indicator: StatusIndicator) -> &'static str {
    match indicator {
        StatusIndicator::None => "none",
        StatusIndicator::Minor => "minor",
        StatusIndicator::Major => "major",
        StatusIndicator::Critical => "critical",
        StatusIndicator::Maintenance => "maintenance",
        StatusIndicator::Unknown => "unknown",
    }
}

fn status_label(indicator: &str) -> &'static str {
    match indicator {
        "none" => "Operational",
        "minor" => "Partial outage",
        "major" => "Major outage",
        "critical" => "Critical issue",
        "maintenance" => "Maintenance",
        _ => "Status unknown",
    }
}

fn status_page_url(provider: ProviderId) -> Option<String> {
    match provider {
        ProviderId::Claude => Some("https://status.anthropic.com".to_string()),
        ProviderId::Codex => Some("https://status.openai.com".to_string()),
        ProviderId::Cursor => Some("https://status.cursor.sh".to_string()),
        ProviderId::Copilot => Some("https://www.githubstatus.com".to_string()),
        ProviderId::Antigravity => Some(
            "https://www.google.com/appsstatus/dashboard/products/npdyhgECDJ6tB66MxXyo".to_string(),
        ),
        ProviderId::Kiro => Some("https://status.aws.amazon.com/rss/all.rss".to_string()),
        _ => None,
    }
}

fn print_help() {
    println!(
        "incubar {}\n\nUsage:\n  incubar status [--format text|json] [--provider <id|all>] [--pretty]\n  incubar cost [--format text|json] [--provider <id|all>] [--pretty]\n\nCommands:\n  status  Print provider status indicators\n  cost    Print local cost usage for Claude/Codex\n\nFlags:\n  --format <text|json>  Output format\n  --json               Shortcut for --format json\n  --pretty             Pretty-print JSON output\n  --provider <id|all>  Provider to query\n  --json-output        Use JSON output\n  -h, --help           Show help\n  -V, --version        Show version",
        env!("CARGO_PKG_VERSION")
    );
}
