# awslogs

> A vibe-coded CloudWatch Logs viewer prototype, built to explore AI-assisted programming and dabble in Rust.

This terminal UI experiment lets you run CloudWatch Logs Insights queries, skim through results, and get a feel for the workflow the eventual rewrite will target. It is usable today, but expect rough edges and rapid changes while it shapes the learning journey.

![Main TUI view](screenshots/01.png)

## Highlights
- **Query faster** – Run CloudWatch Logs Insights queries, page through results, and keep a highlight on the current row.
- **Quick filtering** – Apply in-memory include/exclude filters on the fly to trim noisy results.  
  ![Filtering overlay](screenshots/02.png)
- **Rich detail view** – Inspect structured log payloads with formatted JSON inside the TUI.  
  ![Row detail modal](screenshots/03.png)
- **Column picker** – Choose which columns appear in the results without leaving the keyboard.  
  ![Column selector](screenshots/04.png)

## Download & Run

Grab a prebuilt binary from the [Releases](../../releases) page—only Linux and Windows builds are published for now.

Provide AWS credentials in your environment the same way you would for the AWS CLI. Use the up/down arrow keys to flip through profiles.

## Build From Source (Optional)

If you want to tinker, you can build the prototype yourself.