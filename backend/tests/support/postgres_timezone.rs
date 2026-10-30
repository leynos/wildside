//! UTC readiness for embedded PostgreSQL test clusters.

use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use cap_std::fs::Dir;
use color_eyre::eyre::eyre;
use color_eyre::eyre::{Report, Result, WrapErr};
use postgres::error::SqlState;
use postgres::{Config, NoTls};

/// The server's response to setting the UTC timezone.
#[derive(Debug)]
pub(crate) enum ProbeOutcome {
    Ready,
    Unsupported(Report),
}

/// Check UTC before attempting a repair and verify it again afterwards.
///
/// A successful probe leaves the installation untouched. Only an explicitly
/// unsupported UTC timezone permits a repair; all other errors propagate.
///
/// # Examples
///
/// ```rust,ignore
/// ensure_with(|| Ok(ProbeOutcome::Ready), || unreachable!())?;
/// ```
pub(crate) fn ensure_with(
    mut probe: impl FnMut() -> Result<ProbeOutcome>,
    repair: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let original = match probe()? {
        ProbeOutcome::Ready => return Ok(()),
        ProbeOutcome::Unsupported(error) => error,
    };

    if let Err(error) = repair() {
        return Err(original.wrap_err(format!("UTC timezone repair failed: {error:#}")));
    }

    match probe()
        .wrap_err_with(|| format!("UTC timezone probe failed after repair: {original:#}"))?
    {
        ProbeOutcome::Ready => Ok(()),
        ProbeOutcome::Unsupported(error) => Err(error.wrap_err(format!(
            "UTC remains unsupported after repair; initial failure: {original:#}"
        ))),
    }
}

/// Check and, if necessary, repair UTC for one embedded installation.
///
/// # Examples
///
/// ```rust,ignore
/// ensure_utc_ready(&database_url, Path::new("/path/to/postgres"))?;
/// ```
pub(crate) fn ensure_utc_ready(url: &str, installation: &Path) -> Result<()> {
    let config = url
        .parse::<Config>()
        .wrap_err("parse connection for PostgreSQL UTC probe")?;
    ensure_with(|| probe_utc(&config), || repair_bundled_utc(installation))
}

/// Identify the only SQL error eligible for timezone repair.
///
/// # Examples
///
/// ```rust,ignore
/// assert!(is_unsupported_timezone(Some(&SqlState::INVALID_PARAMETER_VALUE)));
/// ```
pub(crate) fn is_unsupported_timezone(code: Option<&SqlState>) -> bool {
    code == Some(&SqlState::INVALID_PARAMETER_VALUE)
}

/// Probe on a separate thread so callers may already be inside a Tokio runtime.
///
/// # Examples
///
/// ```rust,ignore
/// let config = database_url.parse::<postgres::Config>()?;
/// let outcome = probe_utc(&config)?;
/// ```
pub(crate) fn probe_utc(config: &Config) -> Result<ProbeOutcome> {
    std::thread::scope(|scope| scope.spawn(|| probe_connection(config)).join())
        .map_err(|_| eyre!("PostgreSQL UTC probe thread panicked"))?
}

fn probe_connection(config: &Config) -> Result<ProbeOutcome> {
    let mut client = config
        .connect(NoTls)
        .wrap_err("connect for PostgreSQL UTC probe")?;
    if let Err(error) = client.batch_execute("SET TIME ZONE 'UTC'") {
        if is_unsupported_timezone(error.code()) {
            return Ok(ProbeOutcome::Unsupported(error.into()));
        }
        return Err(Report::from(error).wrap_err("set PostgreSQL UTC timezone"));
    }

    let row = client
        .query_one("SHOW TimeZone", &[])
        .wrap_err("show PostgreSQL timezone")?;
    let timezone: String = row.try_get(0).wrap_err("read PostgreSQL timezone")?;
    if timezone != "UTC" {
        return Err(eyre!(
            "PostgreSQL reported timezone '{timezone}' after setting UTC"
        ));
    }
    Ok(ProbeOutcome::Ready)
}

fn repair_bundled_utc(installation: &Path) -> Result<()> {
    // `pg_config --sharedir` reports the share directory with symlinks resolved
    // (it derives it from the realpath'd executable), so compare like with like.
    let installation = installation
        .canonicalize()
        .wrap_err("resolve embedded PostgreSQL installation path")?;
    let root = Dir::open_ambient_dir(&installation, cap_std::ambient_authority())
        .wrap_err("open embedded PostgreSQL installation")?;
    let executable = root
        .canonicalize("bin/pg_config")
        .wrap_err("locate installation pg_config")?;
    let executable = installation.join(executable);
    let configure = pg_config_output(&executable, "--configure")?;
    let sharedir = pg_config_output(&executable, "--sharedir")?;
    let relative_share = bundled_share_path(&installation, &configure, &sharedir)?;
    let timezone_dir = root
        .open_dir(relative_share.join("timezone"))
        .wrap_err("open verified bundled PostgreSQL timezone data")?;
    ensure_utc_alias(&timezone_dir).wrap_err("create bundled PostgreSQL UTC alias")
}

fn pg_config_output(executable: &Path, option: &str) -> Result<String> {
    let output = Command::new(executable)
        .arg(option)
        .output()
        .wrap_err("run installation pg_config")?;
    if !output.status.success() {
        return Err(eyre!(
            "installation pg_config {option} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).wrap_err("installation pg_config returned non-UTF-8 output")
}

/// Resolve bundled timezone metadata without accepting system or outside paths.
///
/// # Examples
///
/// ```rust,ignore
/// let relative = bundled_share_path(Path::new("/pg"), "--enable-debug", "/pg/share")?;
/// assert_eq!(relative, Path::new("share"));
/// ```
pub(crate) fn bundled_share_path(
    installation: &Path,
    configure: &str,
    sharedir: &str,
) -> Result<PathBuf> {
    let options: Vec<_> = configure
        .split_whitespace()
        .map(|option| option.trim_matches(['\'', '"']))
        .collect();
    if !options.iter().any(|option| option.starts_with("--")) {
        return Err(eyre!(
            "installation pg_config returned invalid configure metadata"
        ));
    }
    if let Some(option) = options
        .iter()
        .find(|option| option.starts_with("--with-system-tzdata"))
    {
        return Err(eyre!(
            "PostgreSQL uses system timezone data ({option}); repair the host timezone installation"
        ));
    }
    let sharedir = Path::new(sharedir.trim());
    if !sharedir.is_absolute() {
        return Err(eyre!(
            "installation pg_config returned a non-absolute share directory"
        ));
    }
    let relative = sharedir
        .strip_prefix(installation)
        .wrap_err("PostgreSQL share directory lies outside its installation")?;
    if relative
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(eyre!("PostgreSQL share directory escapes its installation"));
    }
    Ok(relative.to_path_buf())
}

/// Ensure a verified bundled directory contains `UTC`, atomically and idempotently.
///
/// # Examples
///
/// ```rust,ignore
/// ensure_utc_alias(&timezone_dir)?;
/// ```
pub(crate) fn ensure_utc_alias(timezone_dir: &Dir) -> io::Result<()> {
    if timezone_dir.try_exists("UTC")? {
        return Ok(());
    }
    if !timezone_dir.try_exists("UCT")? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither UTC nor its UCT equivalent exists in PostgreSQL timezone data",
        ));
    }
    match timezone_dir.hard_link("UCT", timezone_dir, "UTC") {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists
                && timezone_dir.try_exists("UTC")? =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
