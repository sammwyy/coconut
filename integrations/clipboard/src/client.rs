use crate::{Entry, Status, INTERFACE, PATH, SERVICE};

#[derive(Clone)]
pub struct Client {
    connection: zbus::blocking::Connection,
}

impl Client {
    pub fn session() -> zbus::Result<Self> {
        Ok(Self {
            connection: zbus::blocking::connection::Builder::session()?
                .method_timeout(std::time::Duration::from_secs(380))
                .build()?,
        })
    }

    fn proxy(&self) -> zbus::Result<zbus::blocking::Proxy<'_>> {
        zbus::blocking::proxy::Builder::new(&self.connection)
            .destination(SERVICE)?
            .path(PATH)?
            .interface(INTERFACE)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
    }

    pub fn status(&self) -> zbus::Result<Status> {
        self.proxy()?.call("Status", &())
    }
    pub fn get_config(&self) -> zbus::Result<(u32, u32, u32, u64)> {
        self.proxy()?.call("GetConfig", &())
    }
    pub fn configure(
        &self,
        max_entries: u32,
        max_item_bytes: u32,
        max_total_bytes: u32,
        max_age_seconds: u64,
    ) -> zbus::Result<()> {
        self.proxy()?.call(
            "Configure",
            &(
                max_entries,
                max_item_bytes,
                max_total_bytes,
                max_age_seconds,
            ),
        )
    }
    pub fn current(&self) -> zbus::Result<(bool, String)> {
        self.proxy()?.call("Current", &())
    }
    pub fn list(&self, offset: u32, limit: u32) -> zbus::Result<Vec<Entry>> {
        self.proxy()?.call("List", &(offset, limit))
    }
    pub fn search(&self, query: &str, offset: u32, limit: u32) -> zbus::Result<Vec<Entry>> {
        self.proxy()?.call("Search", &(query, offset, limit))
    }
    pub fn get(&self, id: u64) -> zbus::Result<String> {
        self.proxy()?.call("Get", &(id,))
    }
    pub fn set_text(&self, text: &str) -> zbus::Result<()> {
        self.proxy()?.call("SetText", &(text,))
    }
    pub fn restore(&self, id: u64) -> zbus::Result<()> {
        self.proxy()?.call("Restore", &(id,))
    }
    pub fn remove(&self, id: u64) -> zbus::Result<bool> {
        self.proxy()?.call("Remove", &(id,))
    }
    pub fn clear(&self) -> zbus::Result<()> {
        self.proxy()?.call("Clear", &())
    }
    pub fn set_enabled(&self, enabled: bool) -> zbus::Result<()> {
        self.proxy()?.call("SetEnabled", &(enabled,))
    }
    pub fn wait_changed(&self, revision: u64) -> zbus::Result<u64> {
        self.proxy()?.call("WaitChanged", &(revision,))
    }
}
