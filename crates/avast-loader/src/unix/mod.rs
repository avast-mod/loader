use color_eyre::eyre;
use avast::Avast;
use std::sync::atomic::{AtomicBool, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[ctor::ctor]
pub fn main() {
    if avast::is_disabled() {
        return;
    }

    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
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
