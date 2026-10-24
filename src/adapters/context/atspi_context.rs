use std::sync::{Arc, Mutex};
use atspi::{AccessibilityConnection, State, ObjectRef};
use atspi::events::object::StateChangedEvent;
use atspi::proxy::text::TextProxy;
use atspi::proxy::zbus::CacheProperties;
use futures_util::StreamExt;
use crate::domain::ports::context::ContextReader;

const MAX_CONTEXT_CHARS: i32 = 2000;

pub struct AtspiContext {
    current_text: Arc<Mutex<Option<String>>>,
}

impl AtspiContext {
    /// Spawn a background task that tracks focused text via AT-SPI.
    /// Returns None if AT-SPI is not available on this system.
    pub async fn spawn() -> Option<Self> {
        let conn = AccessibilityConnection::new().await
            .map_err(|e| tracing::warn!("AT-SPI unavailable: {e}"))
            .ok()?;

        if let Err(e) = conn.register_event::<StateChangedEvent>().await {
            tracing::warn!("AT-SPI register event failed: {e}");
            return None;
        }

        let current_text = Arc::new(Mutex::new(None::<String>));
        let ct = current_text.clone();

        tokio::spawn(async move {
            let mut stream = conn.event_stream();
            while let Some(Ok(event)) = stream.next().await {
                let Ok(ev) = StateChangedEvent::try_from(event) else { continue };
                if ev.enabled == 0 || ev.state != State::Focused { continue }
                let text = read_focused_text(conn.connection(), &ev.item).await;
                *ct.lock().unwrap() = text;
            }
        });

        tracing::info!("AT-SPI context reader active");
        Some(Self { current_text })
    }
}

impl ContextReader for AtspiContext {
    fn get_context(&self) -> Option<String> {
        self.current_text.lock().unwrap().clone()
    }
}

async fn read_focused_text(
    conn: &atspi::proxy::zbus::Connection,
    item: &ObjectRef,
) -> Option<String> {
    let proxy = TextProxy::builder(conn)
        .destination(item.name.as_str()).ok()?
        .path(item.path.as_ref()).ok()?
        .cache_properties(CacheProperties::No)
        .build().await.ok()?;

    let total = proxy.character_count().await.ok()?;
    let cursor = proxy.caret_offset().await.ok()?;
    if total == 0 || cursor == 0 { return None; }

    let start = (cursor - MAX_CONTEXT_CHARS).max(0);
    let text = proxy.get_text(start, cursor).await.ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}
