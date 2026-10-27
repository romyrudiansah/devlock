use anyhow::{Context, Result};
use devlock_core::launcher::{self, HostsSync};
use devlock_core::scaffold::{self, Options, Template};
use devlock_core::{Config, Paths};

pub fn list() -> Result<()> {
    println!("Template yang tersedia:");
    for t in Template::ALL {
        let runtime = match (t.uses_php(), t.uses_node()) {
            (true, _) => "PHP",
            (_, true) => "Node",
            _ => "-",
        };
        let db = if t.needs_database() {
            ", butuh database"
        } else {
            ""
        };
        println!("  {:<12} {} ({runtime}{db})", t.id(), t.label());
    }
    Ok(())
}

pub fn create(
    template: Option<&str>,
    name: Option<&str>,
    php: Option<String>,
    node: Option<String>,
    database: Option<String>,
) -> Result<()> {
    let template: Template = template
        .context("sebutkan template, mis. `devlock new laravel toko` (lihat `devlock new --list`)")?
        .parse()?;
    let name = name.context("sebutkan nama project, mis. `devlock new laravel toko`")?;
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let opts = Options {
        name: name.to_string(),
        php,
        node,
        database,
    };
    let created = scaffold::create(&paths, &config, template, &opts, &mut |line| {
        println!("  {line}");
    })?;

    println!("\n{} siap di {}", template.label(), created.dir.display());
    println!("URL      : {}", created.url);
    if let Some(db) = &created.database {
        println!("Database : {db}");
    }
    // Domain baru perlu masuk file hosts; kegagalan (mis. UAC ditolak) hanya peringatan.
    match launcher::sync_hosts(&paths, &config, false) {
        Ok(HostsSync::Updated { .. }) => println!("File hosts diperbarui."),
        Ok(HostsSync::Unchanged) => {}
        Err(err) => println!("peringatan: file hosts tidak diperbarui ({err:#})"),
    }
    for note in created
        .notes
        .iter()
        .filter(|n| !n.contains("devlock hosts"))
    {
        println!("- {note}");
    }
    Ok(())
}
