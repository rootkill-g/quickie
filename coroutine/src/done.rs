use log::{debug, error};

use crate::event::EventSource;

pub struct Done;

impl Done {
    pub(crate) fn drop_coroutine(coroutine: CoroutineImpl) {
        let local = unsafe { Box::from_raw(get_local_coroutine(&coroutine)) };
        let name = local.get_coroutine().name();

        // Recycle the coroutine
        let (size, used) = coroutine.stack_usage();

        if used == size {
            error!("Stack overflow detected, size = {}", size);
            std::process::exit(1);
        }

        // Show the actual used stack size in debug log
        if local.get_coroutine().stack_size() & 1 == 1 {
            debug!(
                "Coroutine name = {:?}, stack size = {}, used size = {}",
                name, size, used
            );
        }

        if size == config().get_stack_size() {
            get_scheduler().pool.put(coroutine);
        }
    }
}

impl EventSource for Done {
    fn subscribe(&mut self, coroutine_impl: CoroutineImpl) {
        Done::drop_coroutine(coroutine_impl);
    }
}
