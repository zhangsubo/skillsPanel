use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

pub struct InstallCancelRegistry {
    entries: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl InstallCancelRegistry {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, key: &str) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut entries = self.entries.lock().unwrap();
        entries.insert(key.to_string(), cancel.clone());
        cancel
    }

    pub fn cancel(&self, key: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        if let Some(cancel) = entries.remove(key) {
            cancel.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn unregister(&self, key: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key);
    }
}

pub struct CancelRegistrationGuard {
    registry: Arc<InstallCancelRegistry>,
    key: String,
}

impl CancelRegistrationGuard {
    pub fn new(registry: Arc<InstallCancelRegistry>, key: String) -> Self {
        Self { registry, key }
    }
}

impl Drop for CancelRegistrationGuard {
    fn drop(&mut self) {
        self.registry.unregister(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_register_and_cancel() {
        let registry = Arc::new(InstallCancelRegistry::new());
        let cancel = registry.register("test-key");
        assert!(!cancel.load(Ordering::SeqCst));

        let result = registry.cancel("test-key");
        assert!(result);
        assert!(cancel.load(Ordering::SeqCst));
    }

    #[test]
    fn test_cancel_unknown_key() {
        let registry = Arc::new(InstallCancelRegistry::new());
        let result = registry.cancel("nonexistent");
        assert!(!result);
    }

    #[test]
    fn test_guard_auto_unregister() {
        let registry = Arc::new(InstallCancelRegistry::new());
        {
            let cancel = registry.register("guard-key");
            let _guard = CancelRegistrationGuard::new(registry.clone(), "guard-key".into());
            assert!(!cancel.load(Ordering::SeqCst));
        }
        let result = registry.cancel("guard-key");
        assert!(!result);
    }

    #[test]
    fn test_multiple_registrations() {
        let registry = Arc::new(InstallCancelRegistry::new());
        let c1 = registry.register("key1");
        let c2 = registry.register("key2");

        registry.cancel("key1");
        assert!(c1.load(Ordering::SeqCst));
        assert!(!c2.load(Ordering::SeqCst));
    }
}
