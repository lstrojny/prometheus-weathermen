use log::debug;

pub fn init(level: log::Level) -> anyhow::Result<()> {
    log::set_max_level(level.to_level_filter());

    #[cfg(target_os = "linux")]
    if systemd_journal_logger::connected_to_journal() {
        debug!("Initialize systemd journal logger with level {level:?}");

        return Ok(systemd_journal_logger::JournalLog::new()?
            .with_extra_fields(vec![("VERSION", crate::config::VERSION)])
            .install()?);
    }

    debug!("Initialize stderr logger with level {level:?}");

    Ok(simple_logger::init_with_level(level)?)
}
