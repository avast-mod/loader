use color_eyre::eyre;
use avast::Avast;

#[ctor::ctor]
pub fn main() {
    if avast::is_disabled() {
        return;
    }

    // For some reason this function is being called multiple times, so jank workaround for now
    unsafe {
        if std::env::var("AVAST_INIT").is_ok() {
            return;
        }

        std::env::set_var("AVAST_INIT", "1");
    }

    let result = (|| -> color_eyre::Result<()> {
        let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
            .theme(color_eyre::config::Theme::new())
            .into_hooks();
        eyre::set_hook(eyre_hook.into_eyre_hook()).ok();

        Avast::setup_instance_logging_etc()?;
        let instance = Avast::instance();
        if let Err(err) = instance.finish_setup() {
            panic!("Avast initialization failed: {err:?}");
        }

        Ok(())
    })();

    if let Err(e) = result {
        eprintln!("{:?}", e);
    }
}
